use shared::protocol::CloseReasonKind;
use shared::protocol::close_reason;

const LOADING_TEXT: &str = "Loading";
const CONNECTING_TEXT: &str = "Connecting to the game server";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionStatusKind {
    /// The client bundle has not started, as in the prerendered page.
    Loading,
    Connecting,
    Open,
    /// `None` for an abnormal close, a failed open or a close code no close reason uses.
    Closed {
        close_reason: Option<CloseReasonKind>,
    },
    ClosedByClient,
}

impl ConnectionStatusKind {
    /// Host and Join need the server.
    pub fn lobby_access(self) -> MenuButtonAccessKind {
        match self {
            ConnectionStatusKind::Open => MenuButtonAccessKind::Enabled,
            ConnectionStatusKind::Loading
            | ConnectionStatusKind::Connecting
            | ConnectionStatusKind::Closed { .. }
            | ConnectionStatusKind::ClosedByClient => MenuButtonAccessKind::Disabled,
        }
    }

    /// The tutorial runs locally, so it needs only the client bundle.
    pub fn tutorial_access(self) -> MenuButtonAccessKind {
        match self {
            ConnectionStatusKind::Loading => MenuButtonAccessKind::Disabled,
            ConnectionStatusKind::Connecting
            | ConnectionStatusKind::Open
            | ConnectionStatusKind::Closed { .. }
            | ConnectionStatusKind::ClosedByClient => MenuButtonAccessKind::Enabled,
        }
    }

    /// `None` while open. A version mismatch needs a reload, which reconnecting would not give.
    pub fn status_box(self) -> Option<StatusBoxView> {
        match self {
            ConnectionStatusKind::Loading => Some(StatusBoxView::in_progress(LOADING_TEXT)),
            ConnectionStatusKind::Connecting => Some(StatusBoxView::in_progress(CONNECTING_TEXT)),
            ConnectionStatusKind::Open => None,
            ConnectionStatusKind::Closed {
                close_reason: Some(CloseReasonKind::ProtocolVersionMismatch),
            } => Some(StatusBoxView::notice(String::from(
                CloseReasonKind::ProtocolVersionMismatch.client_notice(),
            ))),
            ConnectionStatusKind::Closed { close_reason } => Some(StatusBoxView::closed(
                close_reason.map_or(close_reason::UNREACHABLE_CLIENT_NOTICE, CloseReasonKind::client_notice),
            )),
            ConnectionStatusKind::ClosedByClient => {
                Some(StatusBoxView::closed(close_reason::DISCONNECTED_CLIENT_NOTICE))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuButtonAccessKind {
    Enabled,
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReconnectOfferKind {
    Offered,
    NotOffered,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityIndicatorKind {
    Shown,
    Hidden,
}

/// The title's status box.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusBoxView {
    pub text: String,
    pub activity_indicator: ActivityIndicatorKind,
    pub reconnect_offer: ReconnectOfferKind,
}

impl StatusBoxView {
    pub fn notice(text: String) -> StatusBoxView {
        StatusBoxView {
            text,
            activity_indicator: ActivityIndicatorKind::Hidden,
            reconnect_offer: ReconnectOfferKind::NotOffered,
        }
    }

    fn in_progress(text: &str) -> StatusBoxView {
        StatusBoxView {
            text: String::from(text),
            activity_indicator: ActivityIndicatorKind::Shown,
            reconnect_offer: ReconnectOfferKind::NotOffered,
        }
    }

    fn closed(text: &str) -> StatusBoxView {
        StatusBoxView {
            text: String::from(text),
            activity_indicator: ActivityIndicatorKind::Hidden,
            reconnect_offer: ReconnectOfferKind::Offered,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_closed_status_box(text: &str, reconnect_offer: ReconnectOfferKind) -> Option<StatusBoxView> {
        Some(StatusBoxView {
            text: String::from(text),
            activity_indicator: ActivityIndicatorKind::Hidden,
            reconnect_offer,
        })
    }

    #[test]
    fn lobby_access_is_enabled_only_while_open() {
        assert_eq!(ConnectionStatusKind::Open.lobby_access(), MenuButtonAccessKind::Enabled);
        assert_eq!(
            ConnectionStatusKind::Loading.lobby_access(),
            MenuButtonAccessKind::Disabled,
        );
        assert_eq!(
            ConnectionStatusKind::Connecting.lobby_access(),
            MenuButtonAccessKind::Disabled,
        );
        assert_eq!(
            ConnectionStatusKind::Closed { close_reason: None }.lobby_access(),
            MenuButtonAccessKind::Disabled,
        );
        assert_eq!(
            ConnectionStatusKind::ClosedByClient.lobby_access(),
            MenuButtonAccessKind::Disabled,
        );
    }

    #[test]
    fn tutorial_access_is_disabled_only_while_loading() {
        assert_eq!(
            ConnectionStatusKind::Loading.tutorial_access(),
            MenuButtonAccessKind::Disabled,
        );
        assert_eq!(
            ConnectionStatusKind::Connecting.tutorial_access(),
            MenuButtonAccessKind::Enabled,
        );
        assert_eq!(
            ConnectionStatusKind::Open.tutorial_access(),
            MenuButtonAccessKind::Enabled
        );
        assert_eq!(
            ConnectionStatusKind::Closed { close_reason: None }.tutorial_access(),
            MenuButtonAccessKind::Enabled,
        );
        assert_eq!(
            ConnectionStatusKind::ClosedByClient.tutorial_access(),
            MenuButtonAccessKind::Enabled,
        );
    }

    #[test]
    fn status_box_animates_while_loading_and_while_connecting() {
        assert_eq!(
            ConnectionStatusKind::Loading.status_box(),
            Some(StatusBoxView {
                text: String::from("Loading"),
                activity_indicator: ActivityIndicatorKind::Shown,
                reconnect_offer: ReconnectOfferKind::NotOffered,
            }),
        );
        assert_eq!(
            ConnectionStatusKind::Connecting.status_box(),
            Some(StatusBoxView {
                text: String::from("Connecting to the game server"),
                activity_indicator: ActivityIndicatorKind::Shown,
                reconnect_offer: ReconnectOfferKind::NotOffered,
            }),
        );
    }

    #[test]
    fn status_box_is_absent_while_open() {
        assert_eq!(ConnectionStatusKind::Open.status_box(), None);
    }

    #[test]
    fn status_box_after_a_close_tells_the_close_reason_and_offers_reconnecting() {
        assert_eq!(
            ConnectionStatusKind::Closed { close_reason: None }.status_box(),
            create_closed_status_box("Cannot reach the game server", ReconnectOfferKind::Offered),
        );
        assert_eq!(
            ConnectionStatusKind::Closed {
                close_reason: Some(CloseReasonKind::SlowConsumer),
            }
            .status_box(),
            create_closed_status_box(
                "Disconnected: the connection could not keep up",
                ReconnectOfferKind::Offered,
            ),
        );
        assert_eq!(
            ConnectionStatusKind::Closed {
                close_reason: Some(CloseReasonKind::ServerShutdown),
            }
            .status_box(),
            create_closed_status_box("Disconnected from the game server", ReconnectOfferKind::Offered),
        );
        assert_eq!(
            ConnectionStatusKind::ClosedByClient.status_box(),
            create_closed_status_box("Disconnected from the game server", ReconnectOfferKind::Offered),
        );
    }

    #[test]
    fn status_box_after_a_version_mismatch_offers_no_reconnecting() {
        assert_eq!(
            ConnectionStatusKind::Closed {
                close_reason: Some(CloseReasonKind::ProtocolVersionMismatch),
            }
            .status_box(),
            create_closed_status_box(
                "Bacter has been updated. Reload the page.",
                ReconnectOfferKind::NotOffered,
            ),
        );
    }

    #[test]
    fn notice_shows_the_text_without_animation_or_reconnecting() {
        assert_eq!(
            StatusBoxView::notice(String::from("Lost synchronisation with the game")),
            StatusBoxView {
                text: String::from("Lost synchronisation with the game"),
                activity_indicator: ActivityIndicatorKind::Hidden,
                reconnect_offer: ReconnectOfferKind::NotOffered,
            },
        );
    }
}
