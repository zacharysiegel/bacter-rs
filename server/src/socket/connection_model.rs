use std::sync::atomic::{AtomicU64, Ordering};

use bytes::Bytes;
use tokio::sync::{mpsc, watch};

use shared::game::GameId;
use shared::member::MemberId;
use shared::protocol;
use shared::protocol::{CloseReasonKind, MessageSerialOut};

use crate::game::ConnectionEndpoint;
use crate::registries::ServerRegistries;

pub const FIRST_CONNECTION_ID: u64 = 1;
pub const MEMBERSHIP_CHANNEL_CAPACITY: usize = 256;

pub static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(FIRST_CONNECTION_ID);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionPhase {
    Lobby,
    InGame { game_id: GameId, member_id: MemberId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionControl {
    Open,
    Close(CloseCauseKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolViolationKind {
    Decode,
    Conversion,
    TextFrame,
    Oversize,
    Validation,
}

impl ProtocolViolationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ProtocolViolationKind::Decode => "decode",
            ProtocolViolationKind::Conversion => "conversion",
            ProtocolViolationKind::TextFrame => "text frame",
            ProtocolViolationKind::Oversize => "oversize",
            ProtocolViolationKind::Validation => "validation",
        }
    }
}

/// Why the server closes a connection, with the cause of a protocol violation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseCauseKind {
    Reason(CloseReasonKind),
    ProtocolViolation(ProtocolViolationKind),
    /// Answers the client's own close.
    ClientClosed,
}

impl CloseCauseKind {
    pub fn close_code(self) -> u16 {
        match self {
            CloseCauseKind::Reason(close_reason) => close_reason.close_code(),
            CloseCauseKind::ProtocolViolation(_) => CloseReasonKind::ProtocolViolation.close_code(),
            CloseCauseKind::ClientClosed => protocol::NORMAL_CLOSURE_CODE,
        }
    }

    pub fn description(self) -> Option<String> {
        match self {
            CloseCauseKind::Reason(close_reason) => close_reason.reason_text(),
            CloseCauseKind::ProtocolViolation(violation) => Some(String::from(violation.as_str())),
            CloseCauseKind::ClientClosed => None,
        }
    }
}

/// The receiving ends of a membership, kept by the connection.
pub struct Membership {
    pub outbound: mpsc::Receiver<Bytes>,
    pub control: watch::Receiver<ConnectionControl>,
}

pub fn encode_frame(message_serial_out: &MessageSerialOut) -> Bytes {
    Bytes::from(protocol::encode_message_out(message_serial_out))
}

pub fn allocate_connection_id(registries: ServerRegistries) -> ConnectionId {
    ConnectionId(registries.next_connection_id.fetch_add(1, Ordering::SeqCst))
}

pub fn create_membership(connection_id: ConnectionId) -> (ConnectionEndpoint, Membership) {
    let (outbound_sender, outbound_receiver): (mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) =
        mpsc::channel(MEMBERSHIP_CHANNEL_CAPACITY);
    let (control_sender, control_receiver): (watch::Sender<ConnectionControl>, watch::Receiver<ConnectionControl>) =
        watch::channel(ConnectionControl::Open);

    let endpoint: ConnectionEndpoint = ConnectionEndpoint {
        connection_id,
        outbound: outbound_sender,
        control: control_sender,
    };
    let membership: Membership = Membership {
        outbound: outbound_receiver,
        control: control_receiver,
    };

    (endpoint, membership)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_code_maps_every_cause() {
        assert_eq!(CloseCauseKind::Reason(CloseReasonKind::SlowConsumer).close_code(), 4002);
        assert_eq!(
            CloseCauseKind::ProtocolViolation(ProtocolViolationKind::TextFrame).close_code(),
            1008
        );
        assert_eq!(CloseCauseKind::ClientClosed.close_code(), 1000);
    }

    #[test]
    fn description_names_the_protocol_violation_cause() {
        assert_eq!(
            CloseCauseKind::ProtocolViolation(ProtocolViolationKind::Oversize).description(),
            Some(String::from("oversize")),
        );
        assert_eq!(
            CloseCauseKind::Reason(CloseReasonKind::InboundTimeout).description(),
            Some(String::from("inbound timeout")),
        );
        assert_eq!(
            CloseCauseKind::Reason(CloseReasonKind::InternalError).description(),
            None
        );
        assert_eq!(CloseCauseKind::ClientClosed.description(), None);
    }

    #[test]
    fn create_membership_connects_the_endpoint_to_the_membership() {
        let (endpoint, mut membership): (ConnectionEndpoint, Membership) = create_membership(ConnectionId(3));

        endpoint.outbound.try_send(Bytes::from_static(b"frame")).unwrap();
        endpoint.control.send(ConnectionControl::Close(CloseCauseKind::ClientClosed)).unwrap();

        assert_eq!(endpoint.connection_id, ConnectionId(3));
        assert_eq!(membership.outbound.try_recv().unwrap(), Bytes::from_static(b"frame"));
        assert_eq!(
            *membership.control.borrow(),
            ConnectionControl::Close(CloseCauseKind::ClientClosed)
        );
    }

    #[test]
    fn encode_frame_encodes_the_message() {
        let frame: Bytes = encode_frame(&MessageSerialOut::LeftGame);

        assert_eq!(
            protocol::decode_message_out(&frame).unwrap(),
            MessageSerialOut::LeftGame
        );
    }

    #[test]
    fn allocate_connection_id_counts_up_from_the_first_connection_id() {
        let registries: ServerRegistries = ServerRegistries::leaked();

        assert_eq!(allocate_connection_id(registries), ConnectionId(FIRST_CONNECTION_ID));
        assert_eq!(
            allocate_connection_id(registries),
            ConnectionId(FIRST_CONNECTION_ID + 1)
        );
    }
}
