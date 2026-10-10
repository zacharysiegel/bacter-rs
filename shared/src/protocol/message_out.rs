use bitcode::{Decode, Encode};

use crate::protocol::{
    GameStateSerialOut, GameSummarySerialOut, InputBundleSerialOut, RejectionKindSerialOut, RequestKindSerialOut,
    SubpixelPointSerial,
};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum MessageSerialOut {
    GameList {
        game_summaries: Vec<GameSummarySerialOut>,
        online_client_count: u32,
    },
    JoinAccepted {
        game_id: u32,
        member_id: u32,
    },
    Snapshot(GameSnapshotSerialOut),
    InputBundle(InputBundleSerialOut),
    StateChecksum {
        tick: u32,
        checksum: u64,
    },
    CursorCorrected {
        client_tick: u32,
        cursor: SubpixelPointSerial,
    },
    RequestRejected {
        request: RequestKindSerialOut,
        reason: RejectionKindSerialOut,
    },
    LeftGame,
    GameEnded,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct GameSnapshotSerialOut {
    pub game_id: u32,
    pub state: GameStateSerialOut,
}
