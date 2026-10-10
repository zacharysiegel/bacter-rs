use crate::protocol;

/// For an abnormal close, a failed open, or a close code no close reason uses.
pub const UNREACHABLE_CLIENT_NOTICE: &str = "Cannot reach the game server";
const DISCONNECTED_CLIENT_NOTICE: &str = "Disconnected from the game server";
const CLOSE_REASON_KINDS: [CloseReasonKind; 9] = [
    CloseReasonKind::ServerShutdown,
    CloseReasonKind::ProtocolViolation,
    CloseReasonKind::InternalError,
    CloseReasonKind::ProtocolVersionMismatch,
    CloseReasonKind::SlowConsumer,
    CloseReasonKind::PasswordFailureLimit,
    CloseReasonKind::RateLimitAbuse,
    CloseReasonKind::InboundTimeout,
    CloseReasonKind::ServerFull,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReasonKind {
    ServerShutdown,
    ProtocolViolation,
    InternalError,
    ProtocolVersionMismatch,
    SlowConsumer,
    PasswordFailureLimit,
    RateLimitAbuse,
    InboundTimeout,
    ServerFull,
}

impl CloseReasonKind {
    pub fn close_code(self) -> u16 {
        match self {
            CloseReasonKind::ServerShutdown => 1001,
            CloseReasonKind::ProtocolViolation => 1008,
            CloseReasonKind::InternalError => 1011,
            CloseReasonKind::ProtocolVersionMismatch => 4001,
            CloseReasonKind::SlowConsumer => 4002,
            CloseReasonKind::PasswordFailureLimit => 4003,
            CloseReasonKind::RateLimitAbuse => 4004,
            CloseReasonKind::InboundTimeout => 4005,
            CloseReasonKind::ServerFull => 4006,
        }
    }

    pub fn from_close_code(close_code: u16) -> Option<CloseReasonKind> {
        CLOSE_REASON_KINDS.into_iter().find(|close_reason| close_reason.close_code() == close_code)
    }

    /// `None` for `InternalError`, which has no reason, and `ProtocolViolation`, whose reason names its cause.
    pub fn reason_text(self) -> Option<String> {
        match self {
            CloseReasonKind::ServerShutdown => Some(String::from("server shutting down")),
            CloseReasonKind::ProtocolViolation | CloseReasonKind::InternalError => None,
            CloseReasonKind::ProtocolVersionMismatch => {
                Some(format!("server protocol version {}", protocol::PROTOCOL_VERSION))
            }
            CloseReasonKind::SlowConsumer => Some(String::from("slow consumer")),
            CloseReasonKind::PasswordFailureLimit => Some(String::from("password failure limit")),
            CloseReasonKind::RateLimitAbuse => Some(String::from("rate limit")),
            CloseReasonKind::InboundTimeout => Some(String::from("inbound timeout")),
            CloseReasonKind::ServerFull => Some(String::from("server full")),
        }
    }

    pub fn client_notice(self) -> &'static str {
        match self {
            CloseReasonKind::ServerShutdown | CloseReasonKind::InternalError | CloseReasonKind::InboundTimeout => {
                DISCONNECTED_CLIENT_NOTICE
            }
            CloseReasonKind::ProtocolViolation => "Disconnected: protocol error",
            CloseReasonKind::ProtocolVersionMismatch => "Bacter has been updated. Reload the page.",
            CloseReasonKind::SlowConsumer => "Disconnected: the connection could not keep up",
            CloseReasonKind::PasswordFailureLimit => "Too many incorrect passwords",
            CloseReasonKind::RateLimitAbuse => "Too many requests",
            CloseReasonKind::ServerFull => "The server is full",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_code_gives_the_code_of_every_close_reason() {
        let close_codes: Vec<u16> = CLOSE_REASON_KINDS.iter().map(|close_reason| close_reason.close_code()).collect();

        assert_eq!(close_codes, vec![1001, 1008, 1011, 4001, 4002, 4003, 4004, 4005, 4006]);
    }

    #[test]
    fn from_close_code_reads_every_close_code() {
        for close_reason in CLOSE_REASON_KINDS {
            assert_eq!(
                CloseReasonKind::from_close_code(close_reason.close_code()),
                Some(close_reason)
            );
        }
    }

    #[test]
    fn from_close_code_gives_none_for_an_abnormal_close() {
        assert_eq!(CloseReasonKind::from_close_code(1006), None);
        assert_eq!(CloseReasonKind::from_close_code(1000), None);
    }

    #[test]
    fn reason_text_names_the_protocol_version_on_a_mismatch() {
        assert_eq!(
            CloseReasonKind::ProtocolVersionMismatch.reason_text(),
            Some(String::from("server protocol version 1")),
        );
        assert_eq!(
            CloseReasonKind::SlowConsumer.reason_text(),
            Some(String::from("slow consumer"))
        );
        assert_eq!(CloseReasonKind::InternalError.reason_text(), None);
        assert_eq!(CloseReasonKind::ProtocolViolation.reason_text(), None);
    }

    #[test]
    fn client_notice_gives_the_notice_of_every_close_reason() {
        let client_notices: Vec<&str> =
            CLOSE_REASON_KINDS.iter().map(|close_reason| close_reason.client_notice()).collect();

        assert_eq!(
            client_notices,
            vec![
                "Disconnected from the game server",
                "Disconnected: protocol error",
                "Disconnected from the game server",
                "Bacter has been updated. Reload the page.",
                "Disconnected: the connection could not keep up",
                "Too many incorrect passwords",
                "Too many requests",
                "Disconnected from the game server",
                "The server is full",
            ],
        );
    }
}
