use shared::protocol;

const SECURE_PAGE_PROTOCOL: &str = "https:";
const SECURE_WEBSOCKET_SCHEME: &str = "wss:";
const PLAIN_WEBSOCKET_SCHEME: &str = "ws:";
const WEBSOCKET_PATH: &str = "/ws";

/// The parts of the page's location a same-origin socket URL is built from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageOrigin {
    /// With its colon, as `location.protocol` gives it.
    pub protocol: String,
    /// With any port, as `location.host` gives it.
    pub host: String,
}

/// `configured_websocket_url` comes from `game-server.json`; `None` means the page's own origin.
pub fn resolve_websocket_url(configured_websocket_url: Option<&str>, page_origin: &PageOrigin) -> String {
    let websocket_url: String = match configured_websocket_url {
        Some(configured_websocket_url) => String::from(configured_websocket_url),
        None => get_same_origin_websocket_url(page_origin),
    };

    protocol::get_versioned_websocket_url(&websocket_url)
}

fn get_same_origin_websocket_url(page_origin: &PageOrigin) -> String {
    let scheme: &str = if page_origin.protocol == SECURE_PAGE_PROTOCOL {
        SECURE_WEBSOCKET_SCHEME
    } else {
        PLAIN_WEBSOCKET_SCHEME
    };

    format!("{scheme}//{}{WEBSOCKET_PATH}", page_origin.host)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_page_origin(page_protocol: &str) -> PageOrigin {
        PageOrigin {
            protocol: String::from(page_protocol),
            host: String::from("bacter.example:8443"),
        }
    }

    #[test]
    fn resolve_websocket_url_keeps_a_configured_url() {
        assert_eq!(
            resolve_websocket_url(Some("ws://127.0.0.1:3100/ws"), &create_page_origin("http:")),
            protocol::get_versioned_websocket_url("ws://127.0.0.1:3100/ws"),
        );
    }

    #[test]
    fn resolve_websocket_url_follows_the_page_scheme_on_the_same_origin() {
        assert_eq!(
            resolve_websocket_url(None, &create_page_origin("https:")),
            protocol::get_versioned_websocket_url("wss://bacter.example:8443/ws"),
        );
        assert_eq!(
            resolve_websocket_url(None, &create_page_origin("http:")),
            protocol::get_versioned_websocket_url("ws://bacter.example:8443/ws"),
        );
    }
}
