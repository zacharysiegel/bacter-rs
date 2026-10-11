use std::sync::LazyLock;
use std::sync::atomic::{AtomicU32, Ordering};

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;

use shared::game::GameId;
use shared::protocol::{RejectionKind, protocol_limits};

use crate::game::GameHandle;
use crate::registries::ServerRegistries;

pub const FIRST_GAME_ID: u32 = 1;

pub static GAMES: LazyLock<DashMap<GameId, GameHandle>> = LazyLock::new(DashMap::new);
pub static GAME_TITLES: LazyLock<DashMap<String, GameId>> = LazyLock::new(DashMap::new);
pub static NEXT_GAME_ID: AtomicU32 = AtomicU32::new(FIRST_GAME_ID);
pub static GAME_COUNT: AtomicU32 = AtomicU32::new(0);

/// A title and a game slot held for a game not yet registered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameReservation {
    pub game_id: GameId,
    pub title: String,
}

/// The title first, then a game slot; nothing stays reserved on `Err`.
pub fn reserve_game(registries: ServerRegistries, title: &str) -> Result<GameReservation, RejectionKind> {
    let game_id: GameId = GameId(registries.next_game_id.fetch_add(1, Ordering::SeqCst));

    reserve_title(registries, title, game_id)?;

    let slot_reservation: Result<(), RejectionKind> = reserve_game_slot(registries);

    if let Err(rejection) = slot_reservation {
        release_title(registries, title, game_id);

        return Err(rejection);
    }

    Ok(GameReservation {
        game_id,
        title: String::from(title),
    })
}

pub fn cancel_reservation(registries: ServerRegistries, reservation: &GameReservation) {
    release_title(registries, &reservation.title, reservation.game_id);
    registries.game_count.fetch_sub(1, Ordering::SeqCst);
}

pub fn insert_game(registries: ServerRegistries, game_id: GameId, game_handle: GameHandle) {
    registries.games.insert(game_id, game_handle);
}

/// A clone, so no map reference is held across an await.
pub fn get_game(registries: ServerRegistries, game_id: GameId) -> Option<GameHandle> {
    registries.games.get(&game_id).map(|entry| entry.value().clone())
}

/// A newer game holding the same title keeps it.
pub fn remove_game(registries: ServerRegistries, game_id: GameId, title: &str) {
    registries.games.remove(&game_id);
    release_title(registries, title, game_id);
    registries.game_count.fetch_sub(1, Ordering::SeqCst);
}

fn reserve_title(registries: ServerRegistries, title: &str, game_id: GameId) -> Result<(), RejectionKind> {
    match registries.game_titles.entry(String::from(title)) {
        Entry::Occupied(_) => Err(RejectionKind::TitleTaken),
        Entry::Vacant(vacant_entry) => {
            vacant_entry.insert(game_id);

            Ok(())
        }
    }
}

fn release_title(registries: ServerRegistries, title: &str, game_id: GameId) {
    registries.game_titles.remove_if(title, |_, reserved_game_id| *reserved_game_id == game_id);
}

fn reserve_game_slot(registries: ServerRegistries) -> Result<(), RejectionKind> {
    let previous_game_count: u32 = registries.game_count.fetch_add(1, Ordering::SeqCst);

    if previous_game_count >= protocol_limits::GAME_LIMIT_PER_SERVER {
        registries.game_count.fetch_sub(1, Ordering::SeqCst);

        return Err(RejectionKind::ServerFull);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::GameModeKind;

    #[test]
    fn reserve_game_allocates_increasing_game_ids() {
        let registries: ServerRegistries = ServerRegistries::leaked();

        assert_eq!(
            reserve_game(registries, "First").unwrap().game_id,
            GameId(FIRST_GAME_ID)
        );
        assert_eq!(
            reserve_game(registries, "Second").unwrap().game_id,
            GameId(FIRST_GAME_ID + 1)
        );
        assert_eq!(registries.game_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn reserve_game_rejects_a_taken_title_and_keeps_no_slot() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        reserve_game(registries, "Arena").unwrap();

        assert_eq!(reserve_game(registries, "Arena"), Err(RejectionKind::TitleTaken));
        assert_eq!(registries.game_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn reserve_game_rejects_beyond_the_game_limit_and_releases_the_title() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        registries.game_count.store(protocol_limits::GAME_LIMIT_PER_SERVER, Ordering::SeqCst);

        assert_eq!(reserve_game(registries, "Arena"), Err(RejectionKind::ServerFull));
        assert_eq!(
            registries.game_count.load(Ordering::SeqCst),
            protocol_limits::GAME_LIMIT_PER_SERVER
        );
        assert!(registries.game_titles.is_empty());
    }

    #[test]
    fn cancel_reservation_releases_the_title_and_the_slot() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let reservation: GameReservation = reserve_game(registries, "Arena").unwrap();

        cancel_reservation(registries, &reservation);

        assert!(registries.game_titles.is_empty());
        assert_eq!(registries.game_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn get_game_clones_a_registered_handle() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let reservation: GameReservation = reserve_game(registries, "Arena").unwrap();
        insert_game(
            registries,
            reservation.game_id,
            test_fixture::create_game_handle(
                reservation.game_id,
                test_fixture::create_settings(GameModeKind::FreeForAll, 8),
            ),
        );

        assert!(get_game(registries, reservation.game_id).is_some());
        assert!(get_game(registries, GameId(99)).is_none());
    }

    #[test]
    fn remove_game_keeps_a_newer_reservation_of_the_title() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let first_reservation: GameReservation = reserve_game(registries, "Arena").unwrap();
        registries.game_titles.insert(String::from("Arena"), GameId(42));

        remove_game(registries, first_reservation.game_id, "Arena");

        assert_eq!(*registries.game_titles.get("Arena").unwrap(), GameId(42));
        assert_eq!(registries.game_count.load(Ordering::SeqCst), 0);
    }
}
