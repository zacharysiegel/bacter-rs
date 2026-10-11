use std::net::IpAddr;
use std::sync::atomic::{AtomicU32, AtomicU64};

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::mpsc;

use shared::game::GameId;

use crate::game::GameHandle;
use crate::game::game_registry;
use crate::lobby::lobby_broadcast;
use crate::password::PasswordFailureWindow;
use crate::password::password_failure_limit;
use crate::socket::ConnectionId;
use crate::socket::{connection_limit, connection_model};

/// Every in-memory registry of one server.
#[derive(Clone, Copy)]
pub struct ServerRegistries {
    pub games: &'static DashMap<GameId, GameHandle>,
    pub game_titles: &'static DashMap<String, GameId>,
    pub next_game_id: &'static AtomicU32,
    pub game_count: &'static AtomicU32,
    pub next_connection_id: &'static AtomicU64,
    pub connection_count: &'static AtomicU32,
    pub connections_per_ip: &'static DashMap<IpAddr, u32>,
    pub lobby_subscribers: &'static DashMap<ConnectionId, mpsc::Sender<Bytes>>,
    pub password_failures_per_ip: &'static DashMap<IpAddr, PasswordFailureWindow>,
}

impl ServerRegistries {
    pub fn production() -> ServerRegistries {
        ServerRegistries {
            games: &game_registry::GAMES,
            game_titles: &game_registry::GAME_TITLES,
            next_game_id: &game_registry::NEXT_GAME_ID,
            game_count: &game_registry::GAME_COUNT,
            next_connection_id: &connection_model::NEXT_CONNECTION_ID,
            connection_count: &connection_limit::CONNECTION_COUNT,
            connections_per_ip: &connection_limit::CONNECTIONS_PER_IP,
            lobby_subscribers: &lobby_broadcast::LOBBY_SUBSCRIBERS,
            password_failures_per_ip: &password_failure_limit::PASSWORD_FAILURES_PER_IP,
        }
    }

    /// Fresh registries which are never freed, so parallel in-process servers share nothing.
    pub fn leaked() -> ServerRegistries {
        ServerRegistries {
            games: Box::leak(Box::new(DashMap::new())),
            game_titles: Box::leak(Box::new(DashMap::new())),
            next_game_id: Box::leak(Box::new(AtomicU32::new(game_registry::FIRST_GAME_ID))),
            game_count: Box::leak(Box::new(AtomicU32::new(0))),
            next_connection_id: Box::leak(Box::new(AtomicU64::new(connection_model::FIRST_CONNECTION_ID))),
            connection_count: Box::leak(Box::new(AtomicU32::new(0))),
            connections_per_ip: Box::leak(Box::new(DashMap::new())),
            lobby_subscribers: Box::leak(Box::new(DashMap::new())),
            password_failures_per_ip: Box::leak(Box::new(DashMap::new())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn production_points_at_the_statics() {
        let registries: ServerRegistries = ServerRegistries::production();

        assert!(std::ptr::eq(registries.game_titles, &*game_registry::GAME_TITLES));
        assert!(std::ptr::eq(
            registries.connection_count,
            &connection_limit::CONNECTION_COUNT
        ));
        assert!(std::ptr::eq(
            registries.lobby_subscribers,
            &*lobby_broadcast::LOBBY_SUBSCRIBERS
        ));
    }

    #[test]
    fn leaked_gives_registries_which_share_nothing() {
        let first_registries: ServerRegistries = ServerRegistries::leaked();
        let second_registries: ServerRegistries = ServerRegistries::leaked();

        first_registries.game_titles.insert(String::from("Arena"), GameId(1));
        first_registries.game_count.fetch_add(1, Ordering::SeqCst);

        assert!(second_registries.game_titles.is_empty());
        assert_eq!(second_registries.game_count.load(Ordering::SeqCst), 0);
        assert_eq!(
            second_registries.next_game_id.load(Ordering::SeqCst),
            game_registry::FIRST_GAME_ID
        );
    }
}
