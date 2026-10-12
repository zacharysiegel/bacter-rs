use shared::protocol::CloseReasonKind;
use shared::protocol::close_reason;

const CONNECTING_TEXT: &str = "Connecting to the game server";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionStatusKind {
    Connecting,
    Open,
    /// `None` for an abnormal close, a failed open or a close code no close reason uses.
    Closed {
        close_reason: Option<CloseReasonKind>,
    },
}

impl ConnectionStatusKind {
    /// Host and Join need the server.
    pub fn lobby_access(self) -> LobbyAccessKind {
        match self {
            ConnectionStatusKind::Open => LobbyAccessKind::Enabled,
            ConnectionStatusKind::Connecting | ConnectionStatusKind::Closed { .. } => LobbyAccessKind::Disabled,
        }
    }

    /// `None` while open.
    pub fn notice_text(self) -> Option<&'static str> {
        match self {
            ConnectionStatusKind::Connecting => Some(CONNECTING_TEXT),
            ConnectionStatusKind::Open => None,
            ConnectionStatusKind::Closed { close_reason } => {
                Some(close_reason.map_or(close_reason::UNREACHABLE_CLIENT_NOTICE, CloseReasonKind::client_notice))
            }
        }
    }

    /// A version mismatch needs a reload, which reconnecting would not give.
    pub fn reconnect_offer(self) -> ReconnectOfferKind {
        match self {
            ConnectionStatusKind::Closed {
                close_reason: Some(CloseReasonKind::ProtocolVersionMismatch),
            }
            | ConnectionStatusKind::Connecting
            | ConnectionStatusKind::Open => ReconnectOfferKind::NotOffered,
            ConnectionStatusKind::Closed { .. } => ReconnectOfferKind::Offered,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyAccessKind {
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReconnectOfferKind {
    Offered,
    NotOffered,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lobby_access_is_enabled_only_while_open() {
        assert_eq!(ConnectionStatusKind::Open.lobby_access(), LobbyAccessKind::Enabled);
        assert_eq!(
            ConnectionStatusKind::Connecting.lobby_access(),
            LobbyAccessKind::Disabled,
        );
        assert_eq!(
            ConnectionStatusKind::Closed { close_reason: None }.lobby_access(),
            LobbyAccessKind::Disabled,
        );
    }

    #[test]
    fn notice_text_tells_the_connection_state_or_the_close_reason() {
        assert_eq!(
            ConnectionStatusKind::Connecting.notice_text(),
            Some("Connecting to the game server"),
        );
        assert_eq!(ConnectionStatusKind::Open.notice_text(), None);
        assert_eq!(
            ConnectionStatusKind::Closed { close_reason: None }.notice_text(),
            Some("Cannot reach the game server"),
        );
        assert_eq!(
            ConnectionStatusKind::Closed {
                close_reason: Some(CloseReasonKind::SlowConsumer),
            }
            .notice_text(),
            Some("Disconnected: the connection could not keep up"),
        );
    }

    #[test]
    fn reconnect_offer_is_made_after_a_close_other_than_a_version_mismatch() {
        assert_eq!(
            ConnectionStatusKind::Closed { close_reason: None }.reconnect_offer(),
            ReconnectOfferKind::Offered,
        );
        assert_eq!(
            ConnectionStatusKind::Closed {
                close_reason: Some(CloseReasonKind::ServerShutdown),
            }
            .reconnect_offer(),
            ReconnectOfferKind::Offered,
        );
        assert_eq!(
            ConnectionStatusKind::Closed {
                close_reason: Some(CloseReasonKind::ProtocolVersionMismatch),
            }
            .reconnect_offer(),
            ReconnectOfferKind::NotOffered,
        );
        assert_eq!(
            ConnectionStatusKind::Connecting.reconnect_offer(),
            ReconnectOfferKind::NotOffered,
        );
        assert_eq!(
            ConnectionStatusKind::Open.reconnect_offer(),
            ReconnectOfferKind::NotOffered,
        );
    }
}
