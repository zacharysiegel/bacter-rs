use std::time::Duration;

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot, watch};

use shared::ability::Loadout;
use shared::game::{GameSettings, GameSummary, PlayerInput};
use shared::member::{Appearance, Joiner, MemberId, TeamChoiceKind};
use shared::protocol::RejectionKind;

use crate::socket::{ConnectionControl, ConnectionId};

pub const GAME_REQUEST_CHANNEL_CAPACITY: usize = 64;
pub const GAME_INPUT_CHANNEL_CAPACITY: usize = 1024;
/// For join, spectate, respawn and appearance requests.
pub const REQUEST_SEND_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct GameHandle {
    pub requests: mpsc::Sender<GameRequest>,
    pub inputs: mpsc::Sender<GameInput>,
    pub summary: watch::Receiver<GameSummary>,
    pub password_hash: Option<String>,
    pub settings: GameSettings,
}

pub enum GameRequest {
    Join {
        endpoint: ConnectionEndpoint,
        joiner: Joiner,
        reply: oneshot::Sender<Result<MemberId, RejectionKind>>,
    },
    Spectate {
        endpoint: ConnectionEndpoint,
        screen_name: String,
        reply: oneshot::Sender<Result<MemberId, RejectionKind>>,
    },
    Respawn {
        member_id: MemberId,
        loadout: Loadout,
        team: TeamChoiceKind,
        reply: oneshot::Sender<Result<(), RejectionKind>>,
    },
    UpdateAppearance {
        member_id: MemberId,
        appearance: Appearance,
    },
    Leave {
        member_id: MemberId,
        reply: oneshot::Sender<()>,
    },
    ConnectionClosed {
        connection_id: ConnectionId,
    },
    RequestSnapshot {
        member_id: MemberId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameInput {
    pub member_id: MemberId,
    pub input: PlayerInput,
}

/// The sending ends of a membership, owned by the game task.
pub struct ConnectionEndpoint {
    pub connection_id: ConnectionId,
    pub outbound: mpsc::Sender<Bytes>,
    pub control: watch::Sender<ConnectionControl>,
}
