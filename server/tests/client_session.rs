mod helpers;

use std::collections::BTreeMap;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time::{Instant, Interval};

use helpers::test_server;
use helpers::test_server::{TestClient, TestServer};
use shared::error::AppError;
use shared::game::{GameId, GameState, Tick};
use shared::play::{
    ClientEffect, ClientSession, FrameOutput, GameKeyKind, InputFocusKind, KeyRepeatKind, KeyTransitionKind,
    PlayerControls,
};
use shared::protocol;
use shared::protocol::state_checksum;
use shared::protocol::{MessageSerialIn, MessageSerialOut};

/// About one display frame.
const FRAME_PERIOD: Duration = Duration::from_millis(16);
const CREATOR_HEAD_START_TICK: Tick = Tick(3);
const FINAL_TICK: Tick = Tick(60);
const COMMON_TICK_MINIMUM: usize = 40;

enum DriverEventKind {
    FrameReceived(Vec<u8>),
    FrameDue,
}

/// A `ClientSession` connected to the test server, driven as a browser would drive it.
struct SessionDriver {
    client: TestClient,
    session: ClientSession,
    controls: PlayerControls,
    started_at: Instant,
    checksums_by_tick: BTreeMap<Tick, u64>,
    server_checksum_count: u32,
    resync_count: u32,
}

impl SessionDriver {
    async fn connect(test_server: &TestServer, held_keys: &[GameKeyKind]) -> SessionDriver {
        let mut controls: PlayerControls = PlayerControls::new();

        for held_key in held_keys {
            controls.keyboard.key_changed(*held_key, KeyTransitionKind::Pressed, KeyRepeatKind::Initial);
        }

        SessionDriver {
            client: TestClient::connect(test_server).await,
            session: ClientSession::new(),
            controls,
            started_at: Instant::now(),
            checksums_by_tick: BTreeMap::new(),
            server_checksum_count: 0,
            resync_count: 0,
        }
    }

    fn now_milliseconds(&self) -> f64 {
        self.started_at.elapsed().as_secs_f64() * 1000.0
    }

    fn applied_tick(&self) -> Option<Tick> {
        self.session.render_state().map(|state| state.tick)
    }

    async fn send_request(&mut self, request: MessageSerialIn) {
        let effects: Vec<ClientEffect> = self.session.send_request(request);
        self.handle_effects(effects).await;
    }

    async fn play_until_tick(&mut self, final_tick: Tick) {
        tokio::time::timeout(test_server::PLAY_TIMEOUT, self.play_frames_until_tick(final_tick))
            .await
            .expect("the session reaches the final tick within the play timeout");
    }

    async fn play_frames_until_tick(&mut self, final_tick: Tick) {
        let mut frame_interval: Interval = tokio::time::interval(FRAME_PERIOD);

        while self.applied_tick().is_none_or(|applied_tick| applied_tick < final_tick) {
            let driver_event: DriverEventKind = tokio::select! {
                frame = self.client.receive_binary_frame() => DriverEventKind::FrameReceived(frame),
                _ = frame_interval.tick() => DriverEventKind::FrameDue,
            };

            self.handle_driver_event(driver_event).await;
        }
    }

    async fn handle_driver_event(&mut self, driver_event: DriverEventKind) {
        let now_milliseconds: f64 = self.now_milliseconds();

        match driver_event {
            DriverEventKind::FrameReceived(frame) => {
                let effects: Vec<ClientEffect> = self.session.receive(&frame, &mut self.controls, now_milliseconds);
                self.handle_effects(effects).await;
                self.verify_server_checksum(&frame);
            }
            DriverEventKind::FrameDue => {
                let frame_output: FrameOutput =
                    self.session.advance_frame(now_milliseconds, &mut self.controls, InputFocusKind::Game);
                self.handle_effects(frame_output.effects).await;
            }
        }
    }

    async fn handle_effects(&mut self, effects: Vec<ClientEffect>) {
        for effect in effects {
            match effect {
                ClientEffect::SendFrame(frame) => self.client.send_raw(frame.into()).await,
                ClientEffect::TickApplied { tick } => self.record_checksum(tick),
                ClientEffect::ResyncStarted | ClientEffect::SynchronisationLost => self.resync_count += 1,
                ClientEffect::RequestRejected { .. }
                | ClientEffect::ConnectionLost(_)
                | ClientEffect::LeftGame
                | ClientEffect::GameEnded => panic!("unexpected effect: {effect:?}"),
                ClientEffect::GameListReceived { .. }
                | ClientEffect::JoinAccepted { .. }
                | ClientEffect::OwnMemberStateChanged(_)
                | ClientEffect::OwnOrganismSpawned { .. }
                | ClientEffect::OwnSpawnRejected
                | ClientEffect::ResyncFinished => {}
            }
        }
    }

    fn record_checksum(&mut self, tick: Tick) {
        let state: &GameState = self.session.render_state().expect("an applied tick has a state");

        assert_eq!(state.tick, tick);
        self.checksums_by_tick.insert(tick, state_checksum::get_state_checksum(state));
    }

    /// A second check, independent of the comparison the session runs itself.
    fn verify_server_checksum(&mut self, frame: &[u8]) {
        let decoded: Result<MessageSerialOut, AppError> = protocol::decode_message_out(frame);

        let Ok(MessageSerialOut::StateChecksum { tick, checksum }) = decoded else {
            return;
        };

        assert_eq!(self.checksums_by_tick.get(&Tick(tick)), Some(&checksum));
        self.server_checksum_count += 1;
    }
}

fn get_common_ticks(first_driver: &SessionDriver, second_driver: &SessionDriver) -> Vec<Tick> {
    first_driver
        .checksums_by_tick
        .keys()
        .filter(|tick| second_driver.checksums_by_tick.contains_key(tick))
        .copied()
        .collect()
}

fn spawn_play_until_tick(mut driver: SessionDriver, final_tick: Tick) -> JoinHandle<SessionDriver> {
    actix_web::rt::spawn(async move {
        driver.play_until_tick(final_tick).await;

        driver
    })
}

#[actix_web::test]
async fn two_client_sessions_compute_identical_checksums_for_one_game() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: SessionDriver =
        SessionDriver::connect(&test_server, &[GameKeyKind::MoveRight, GameKeyKind::FirstAbility]).await;
    creator.send_request(test_server::create_game_message("Arena", None, "Creator")).await;
    creator.play_until_tick(CREATOR_HEAD_START_TICK).await;

    let game_id: GameId = creator.session.game_id().expect("the creator is in its game");
    let mut joiner: SessionDriver = SessionDriver::connect(&test_server, &[GameKeyKind::MoveDown]).await;
    joiner.send_request(test_server::join_game_message(game_id.0, None, "Joiner")).await;

    let creator_task: JoinHandle<SessionDriver> = spawn_play_until_tick(creator, FINAL_TICK);
    let joiner_task: JoinHandle<SessionDriver> = spawn_play_until_tick(joiner, FINAL_TICK);
    let creator: SessionDriver = creator_task.await.expect("the creator plays");
    let joiner: SessionDriver = joiner_task.await.expect("the joiner plays");
    let common_ticks: Vec<Tick> = get_common_ticks(&creator, &joiner);

    assert!(common_ticks.len() >= COMMON_TICK_MINIMUM);

    for tick in &common_ticks {
        assert_eq!(creator.checksums_by_tick[tick], joiner.checksums_by_tick[tick]);
    }

    assert!(creator.server_checksum_count >= 1);
    assert!(joiner.server_checksum_count >= 1);
    assert_eq!((creator.resync_count, joiner.resync_count), (0, 0));
}
