use std::sync::LazyLock;
use std::sync::atomic::AtomicU32;

use dashmap::DashMap;

use shared::game::GameId;

use crate::game::GameHandle;

pub const FIRST_GAME_ID: u32 = 1;

pub static GAMES: LazyLock<DashMap<GameId, GameHandle>> = LazyLock::new(DashMap::new);
pub static GAME_TITLES: LazyLock<DashMap<String, GameId>> = LazyLock::new(DashMap::new);
pub static NEXT_GAME_ID: AtomicU32 = AtomicU32::new(FIRST_GAME_ID);
pub static GAME_COUNT: AtomicU32 = AtomicU32::new(0);
