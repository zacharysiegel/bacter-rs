use std::fs;
use std::path::{Path, PathBuf};

use shared::game;
use shared::game::{GameState, Tick};
use shared::protocol;
use shared::protocol::GameStateSerialOut;
use shared::protocol::state_checksum;
use shared::replay;
use shared::replay::{ReplayLog, TickChecksum};

const SCRIPTED_GAME_TICK_COUNT: usize = 10_000;
const SNAPSHOT_TICK: Tick = Tick(2500);

fn read_scripted_game_log() -> ReplayLog {
    let fixture_path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scripted_game.replay");

    replay::read_replay(&fs::read(fixture_path).unwrap()).unwrap()
}

fn restore_from_snapshot(state: &GameState) -> GameState {
    let snapshot_bytes: Vec<u8> = protocol::encode_game_state(&GameStateSerialOut::from(state));

    GameState::try_from(protocol::decode_game_state(&snapshot_bytes).unwrap()).unwrap()
}

#[test]
fn run_replay_gives_identical_checksums_in_two_runs() {
    let replay_log: ReplayLog = read_scripted_game_log();
    let first_tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log).unwrap();
    let second_tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log).unwrap();

    assert_eq!(first_tick_checksums.len(), SCRIPTED_GAME_TICK_COUNT);
    assert_eq!(first_tick_checksums, second_tick_checksums);
}

#[test]
fn snapshot_round_trip_continues_with_equal_checksums() {
    let replay_log: ReplayLog = read_scripted_game_log();
    let mut state: GameState = GameState::new(replay_log.header.settings.clone(), replay_log.header.seed);
    let mut restored_state: Option<GameState> = None;

    for bundle in &replay_log.input_bundles {
        game::step(&mut state, bundle).unwrap();

        if let Some(restored_state) = restored_state.as_mut() {
            game::step(restored_state, bundle).unwrap();

            assert_eq!(
                state_checksum::get_state_checksum(restored_state),
                state_checksum::get_state_checksum(&state)
            );
        }

        if bundle.tick == SNAPSHOT_TICK {
            let snapshot_state: GameState = restore_from_snapshot(&state);

            assert_eq!(snapshot_state, state);

            restored_state = Some(snapshot_state);
        }
    }

    assert!(restored_state.is_some());
}
