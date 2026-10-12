use shared::game::{GameId, GameState};
use shared::member::MemberId;
use shared::play::{ClientSession, PlayerControls};
use shared::protocol;
use shared::protocol::{GameSnapshotSerialOut, GameStateSerialOut, MessageSerialOut};

pub const FIXTURE_GAME_ID: GameId = GameId(4);

pub fn create_session_joined(own_member_id: MemberId, controls: &mut PlayerControls) -> ClientSession {
    let mut client_session: ClientSession = ClientSession::new();
    let join_accepted: MessageSerialOut = MessageSerialOut::JoinAccepted {
        game_id: FIXTURE_GAME_ID.0,
        member_id: own_member_id.0,
    };

    client_session.receive(&protocol::encode_message_out(&join_accepted), controls, 0.0);

    client_session
}

pub fn create_session_in_game(
    state: &GameState,
    own_member_id: MemberId,
    controls: &mut PlayerControls,
) -> ClientSession {
    let mut client_session: ClientSession = create_session_joined(own_member_id, controls);
    let snapshot: MessageSerialOut = MessageSerialOut::Snapshot(GameSnapshotSerialOut {
        game_id: FIXTURE_GAME_ID.0,
        state: GameStateSerialOut::from(state),
    });

    client_session.receive(&protocol::encode_message_out(&snapshot), controls, 0.0);

    client_session
}
