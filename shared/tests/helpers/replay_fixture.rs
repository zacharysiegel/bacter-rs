use std::fs;
use std::path::{Path, PathBuf};

use shared::replay;
use shared::replay::ReplayLog;

pub fn read_scripted_game_log() -> ReplayLog {
    let fixture_path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scripted_game.replay");

    replay::read_replay(&fs::read(fixture_path).unwrap()).unwrap()
}
