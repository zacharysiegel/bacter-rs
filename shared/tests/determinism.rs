use std::fs;
use std::path::{Path, PathBuf};

use shared::replay;
use shared::replay::{ReplayLog, TickChecksum};

fn read_scripted_game_log() -> ReplayLog {
    let fixture_path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scripted_game.replay");

    replay::read_replay(&fs::read(fixture_path).unwrap()).unwrap()
}

#[test]
fn run_replay_gives_identical_checksums_in_two_runs() {
    let replay_log: ReplayLog = read_scripted_game_log();
    let first_tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log).unwrap();
    let second_tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log).unwrap();

    assert_eq!(first_tick_checksums.len(), 10_000);
    assert_eq!(first_tick_checksums, second_tick_checksums);
}
