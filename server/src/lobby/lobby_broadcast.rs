use std::sync::LazyLock;
use std::time::{Duration, Instant};

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::time::{Interval, MissedTickBehavior};

use crate::lobby::GameList;
use crate::lobby::game_list;
use crate::password::password_failure_limit;
use crate::registries::ServerRegistries;
use crate::socket::ConnectionId;

pub const LOBBY_CHANNEL_CAPACITY: usize = 16;
pub const LOBBY_BROADCAST_MINIMUM_INTERVAL: Duration = Duration::from_secs(1);
const LOBBY_POLL_PERIOD: Duration = Duration::from_millis(250);

pub static LOBBY_SUBSCRIBERS: LazyLock<DashMap<ConnectionId, mpsc::Sender<Bytes>>> = LazyLock::new(DashMap::new);

/// The revision changes whenever a summary or the online client count does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LobbyRevision {
    revision: u64,
    sent_revision: u64,
    last_game_list: Option<GameList>,
    last_sent_at: Option<Instant>,
}

impl LobbyRevision {
    pub fn new() -> LobbyRevision {
        LobbyRevision::default()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn observe(&mut self, game_list: GameList) {
        if self.last_game_list.as_ref() == Some(&game_list) {
            return;
        }

        self.revision += 1;
        self.last_game_list = Some(game_list);
    }

    /// The latest list, when its revision is unsent and the minimum interval has passed.
    pub fn take_due_list(&mut self, now: Instant) -> Option<&GameList> {
        let is_interval_passed: bool = self
            .last_sent_at
            .is_none_or(|last_sent_at| now.saturating_duration_since(last_sent_at) >= LOBBY_BROADCAST_MINIMUM_INTERVAL);

        if self.revision == self.sent_revision || !is_interval_passed {
            return None;
        }

        self.sent_revision = self.revision;
        self.last_sent_at = Some(now);

        self.last_game_list.as_ref()
    }
}

/// Sends the current list at once.
pub fn subscribe(registries: ServerRegistries, connection_id: ConnectionId, lobby_sender: mpsc::Sender<Bytes>) {
    let game_list_frame: Bytes = game_list::get_game_list(registries).encode_frame();
    let _ = lobby_sender.try_send(game_list_frame);
    registries.lobby_subscribers.insert(connection_id, lobby_sender);
}

pub fn unsubscribe(registries: ServerRegistries, connection_id: ConnectionId) {
    registries.lobby_subscribers.remove(&connection_id);
}

pub async fn run_lobby_task(registries: ServerRegistries) {
    let mut interval: Interval = tokio::time::interval(LOBBY_POLL_PERIOD);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut lobby_revision: LobbyRevision = LobbyRevision::new();

    loop {
        interval.tick().await;
        let now: Instant = Instant::now();

        password_failure_limit::forget_expired_failures(registries.password_failures_per_ip, now);

        lobby_revision.observe(game_list::get_game_list(registries));
        let due_game_list: Option<&GameList> = lobby_revision.take_due_list(now);

        if let Some(game_list) = due_game_list {
            broadcast_game_list(registries, &game_list.encode_frame());
        }
    }
}

/// A full subscriber misses this list only; a closed one is removed.
fn broadcast_game_list(registries: ServerRegistries, game_list_frame: &Bytes) {
    registries.lobby_subscribers.retain(|_, lobby_sender| {
        let send_result: Result<(), TrySendError<Bytes>> = lobby_sender.try_send(game_list_frame.clone());

        !matches!(send_result, Err(TrySendError::Closed(_)))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::{GameId, GameModeKind, GameState, GameSummary};
    use shared::protocol;
    use shared::protocol::MessageSerialOut;

    fn create_game_list(player_count: u8, online_client_count: u32) -> GameList {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let mut game_summary: GameSummary = GameSummary::from_game_state(GameId(1), &state, false);
        game_summary.player_count = player_count;

        GameList {
            game_summaries: vec![game_summary],
            online_client_count,
        }
    }

    #[test]
    fn observe_changes_the_revision_on_a_summary_or_client_count_change_only() {
        let mut lobby_revision: LobbyRevision = LobbyRevision::new();

        lobby_revision.observe(create_game_list(0, 1));
        lobby_revision.observe(create_game_list(0, 1));
        lobby_revision.observe(create_game_list(1, 1));
        lobby_revision.observe(create_game_list(1, 2));

        assert_eq!(lobby_revision.revision(), 3);
    }

    #[test]
    fn take_due_list_gives_a_changed_list_at_most_once_per_interval() {
        let mut lobby_revision: LobbyRevision = LobbyRevision::new();
        let start: Instant = Instant::now();
        lobby_revision.observe(create_game_list(0, 1));

        assert_eq!(lobby_revision.take_due_list(start), Some(&create_game_list(0, 1)));
        assert_eq!(
            lobby_revision.take_due_list(start + LOBBY_BROADCAST_MINIMUM_INTERVAL),
            None
        );

        lobby_revision.observe(create_game_list(1, 1));

        assert_eq!(lobby_revision.take_due_list(start + Duration::from_millis(999)), None);
        assert_eq!(
            lobby_revision.take_due_list(start + LOBBY_BROADCAST_MINIMUM_INTERVAL),
            Some(&create_game_list(1, 1)),
        );
    }

    #[test]
    fn subscribe_sends_the_current_list_at_once() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let (lobby_sender, mut lobby_receiver): (mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) =
            mpsc::channel(LOBBY_CHANNEL_CAPACITY);

        subscribe(registries, ConnectionId(1), lobby_sender);

        assert!(matches!(
            protocol::decode_message_out(&lobby_receiver.try_recv().unwrap()).unwrap(),
            MessageSerialOut::GameList { .. },
        ));
        assert!(registries.lobby_subscribers.contains_key(&ConnectionId(1)));

        unsubscribe(registries, ConnectionId(1));

        assert!(registries.lobby_subscribers.is_empty());
    }

    #[test]
    fn broadcast_game_list_skips_a_full_subscriber_and_removes_a_closed_one() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let (full_sender, _full_receiver): (mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) = mpsc::channel(1);
        let (closed_sender, closed_receiver): (mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) = mpsc::channel(1);
        full_sender.try_send(Bytes::from_static(b"older list")).unwrap();
        drop(closed_receiver);
        registries.lobby_subscribers.insert(ConnectionId(1), full_sender);
        registries.lobby_subscribers.insert(ConnectionId(2), closed_sender);

        broadcast_game_list(registries, &Bytes::from_static(b"list"));

        assert!(registries.lobby_subscribers.contains_key(&ConnectionId(1)));
        assert!(!registries.lobby_subscribers.contains_key(&ConnectionId(2)));
    }
}
