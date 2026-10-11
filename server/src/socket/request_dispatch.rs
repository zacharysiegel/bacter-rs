use std::time::Instant;

use shared::protocol::{
    CloseReasonKind, MessageSerialIn, MessageSerialOut, RejectionKind, RejectionKindSerialOut, RequestKind,
    RequestKindSerialOut,
};

use crate::lobby::lobby_broadcast;
use crate::socket::{CloseCauseKind, Connection, ConnectionFlow, ConnectionPhase, TokenTakeKind};

/// Whether a request may proceed; a shut gate has already answered it.
enum RequestGate {
    Open,
    Shut(ConnectionFlow),
}

/// Only the lobby subscription is answered.
pub async fn dispatch(connection: &mut Connection, message_serial_in: MessageSerialIn) -> ConnectionFlow {
    match message_serial_in {
        MessageSerialIn::SubscribeGameList => subscribe_game_list(connection).await,
        MessageSerialIn::UnsubscribeGameList => {
            lobby_broadcast::unsubscribe(connection.registries, connection.connection_id);

            ConnectionFlow::Continue
        }
        MessageSerialIn::CreateGame { .. }
        | MessageSerialIn::JoinGame { .. }
        | MessageSerialIn::SpectateGame { .. }
        | MessageSerialIn::Respawn { .. }
        | MessageSerialIn::UpdateAppearance { .. }
        | MessageSerialIn::Input(_)
        | MessageSerialIn::RequestSnapshot { .. }
        | MessageSerialIn::LeaveGame => ConnectionFlow::Continue,
    }
}

async fn subscribe_game_list(connection: &mut Connection) -> ConnectionFlow {
    let gate: RequestGate = open_lobby_request(connection, RequestKind::SubscribeGameList).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    lobby_broadcast::subscribe(
        connection.registries,
        connection.connection_id,
        connection.lobby_sender.clone(),
    );

    ConnectionFlow::Continue
}

/// Lobby phase only, within the request rate.
async fn open_lobby_request(connection: &mut Connection, request_kind: RequestKind) -> RequestGate {
    if connection.phase() != ConnectionPhase::Lobby {
        return RequestGate::Shut(reject(connection, request_kind, RejectionKind::AlreadyInGame).await);
    }

    take_request_token(connection, request_kind).await
}

/// Sustained abuse closes the connection.
async fn take_request_token(connection: &mut Connection, request_kind: RequestKind) -> RequestGate {
    let request_token: TokenTakeKind = connection.request_bucket.take(Instant::now());

    match request_token {
        TokenTakeKind::Taken => RequestGate::Open,
        TokenTakeKind::Refused => RequestGate::Shut(reject(connection, request_kind, RejectionKind::RateLimited).await),
        TokenTakeKind::Abused => RequestGate::Shut(ConnectionFlow::Close(CloseCauseKind::Reason(
            CloseReasonKind::RateLimitAbuse,
        ))),
    }
}

async fn reject(connection: &Connection, request_kind: RequestKind, rejection: RejectionKind) -> ConnectionFlow {
    let rejection_message: MessageSerialOut = MessageSerialOut::RequestRejected {
        request: RequestKindSerialOut::from(&request_kind),
        reason: RejectionKindSerialOut::from(&rejection),
    };

    connection.send_message(&rejection_message).await
}
