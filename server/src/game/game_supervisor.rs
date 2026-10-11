use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::task::{JoinError, JoinHandle};

use shared::game::{GameSettings, GameState, GameSummary};
use shared::protocol::{GameSettingsSerialOut, ReplayHeaderSerialOut};

use crate::config::ServerConfig;
use crate::game::{GameHandle, GameInput, GameRequest, GameReservation, GameTask, ReplayCapture};
use crate::game::{game_handle, game_registry, game_runtime, game_task};
use crate::registries::ServerRegistries;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameTaskConfig {
    pub tick_period: Duration,
    pub empty_game_grace_period: Duration,
    pub replay_capture_directory: Option<PathBuf>,
}

impl GameTaskConfig {
    pub fn from_server_config(config: &ServerConfig) -> GameTaskConfig {
        GameTaskConfig {
            tick_period: config.tick_period,
            empty_game_grace_period: config.empty_game_grace_period,
            replay_capture_directory: config.replay_capture_directory.clone(),
        }
    }
}

/// The game stays registered until its task ends.
pub fn launch_game(
    registries: ServerRegistries,
    game_config: &GameTaskConfig,
    reservation: &GameReservation,
    settings: GameSettings,
    seed: u64,
    password_hash: Option<String>,
) -> GameHandle {
    let state: GameState = GameState::new(settings.clone(), seed);
    let is_secured: bool = password_hash.is_some();
    let (request_sender, request_receiver): (mpsc::Sender<GameRequest>, mpsc::Receiver<GameRequest>) =
        mpsc::channel(game_handle::GAME_REQUEST_CHANNEL_CAPACITY);
    let (input_sender, input_receiver): (mpsc::Sender<GameInput>, mpsc::Receiver<GameInput>) =
        mpsc::channel(game_handle::GAME_INPUT_CHANNEL_CAPACITY);
    let (summary_sender, summary_receiver): (watch::Sender<GameSummary>, watch::Receiver<GameSummary>) =
        watch::channel(GameSummary::from_game_state(reservation.game_id, &state, is_secured));
    let replay_capture: Option<ReplayCapture> = start_replay_capture(game_config, reservation, &settings, seed);

    let game_task: GameTask = GameTask::new(
        reservation.game_id,
        state,
        is_secured,
        summary_sender,
        replay_capture,
        game_config.empty_game_grace_period,
    );
    let game_handle: GameHandle = GameHandle {
        requests: request_sender,
        inputs: input_sender,
        summary: summary_receiver,
        password_hash,
        settings: settings.clone(),
    };

    game_registry::insert_game(registries, reservation.game_id, game_handle.clone());

    let task_handle: JoinHandle<()> = game_runtime::GAME_RUNTIME.spawn(game_task::run_game_task(
        game_task,
        request_receiver,
        input_receiver,
        game_config.tick_period,
    ));
    game_runtime::GAME_RUNTIME.spawn(supervise_game(registries, reservation.clone(), task_handle));

    log::info!(
        "game created; [game_id={} mode={}]",
        reservation.game_id.0,
        settings.mode.code()
    );

    game_handle
}

fn start_replay_capture(
    game_config: &GameTaskConfig,
    reservation: &GameReservation,
    settings: &GameSettings,
    seed: u64,
) -> Option<ReplayCapture> {
    let directory: &PathBuf = game_config.replay_capture_directory.as_ref()?;
    let header_serial_out: ReplayHeaderSerialOut = ReplayHeaderSerialOut {
        settings: GameSettingsSerialOut::from(settings),
        seed,
    };
    let (replay_capture, _writer_handle): (ReplayCapture, JoinHandle<()>) =
        ReplayCapture::start(directory, reservation.game_id, header_serial_out);

    Some(replay_capture)
}

/// The only code which unregisters a game, however its task ended.
async fn supervise_game(registries: ServerRegistries, reservation: GameReservation, task_handle: JoinHandle<()>) {
    let task_result: Result<(), JoinError> = task_handle.await;

    if let Err(error) = task_result {
        log::error!("game task failed; [game_id={} error={error}]", reservation.game_id.0);
    }

    game_registry::remove_game(registries, reservation.game_id, &reservation.title);

    log::info!("game removed; [game_id={}]", reservation.game_id.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::GameModeKind;
    use std::sync::atomic::Ordering;

    fn wait_for_removal(registries: ServerRegistries) {
        game_runtime::GAME_RUNTIME.block_on(async {
            for _ in 0..500 {
                if registries.games.is_empty() {
                    break;
                }

                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
    }

    fn assert_unregistered(registries: ServerRegistries) {
        assert!(registries.games.is_empty());
        assert!(registries.game_titles.is_empty());
        assert_eq!(registries.game_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn launch_game_registers_the_game_until_it_ends_empty() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let reservation: GameReservation = game_registry::reserve_game(registries, "Fixture game").unwrap();
        let game_config: GameTaskConfig = GameTaskConfig {
            tick_period: Duration::from_millis(5),
            empty_game_grace_period: Duration::from_millis(50),
            replay_capture_directory: None,
        };

        let game_handle: GameHandle = launch_game(
            registries,
            &game_config,
            &reservation,
            test_fixture::create_settings(GameModeKind::FreeForAll, 8),
            1,
            Some(String::from("hash")),
        );

        assert!(game_registry::get_game(registries, reservation.game_id).is_some());
        assert!(game_handle.summary.borrow().secured);

        wait_for_removal(registries);

        assert_unregistered(registries);
    }

    #[test]
    fn supervise_game_unregisters_a_game_whose_task_panicked() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let reservation: GameReservation = game_registry::reserve_game(registries, "Fixture game").unwrap();
        game_registry::insert_game(
            registries,
            reservation.game_id,
            test_fixture::create_game_handle(
                reservation.game_id,
                test_fixture::create_settings(GameModeKind::FreeForAll, 8),
            ),
        );

        let task_handle: JoinHandle<()> = game_runtime::GAME_RUNTIME.spawn(async { panic!("step failed") });
        game_runtime::GAME_RUNTIME.block_on(supervise_game(registries, reservation, task_handle));

        assert_unregistered(registries);
    }
}
