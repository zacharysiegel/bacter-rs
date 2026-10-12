use js_sys::{JSON, Promise, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Response, Window};

use shared::error::AppError;

use crate::connection;
use crate::connection::PageOrigin;

const GAME_SERVER_CONFIG_PATH: &str = "/game-server.json";
const WEBSOCKET_URL_FIELD: &str = "websocket_url";

/// The socket URL `game-server.json` names, versioned; a null URL is the page's own origin.
pub async fn read_websocket_url() -> Result<String, AppError> {
    let window: Window = get_window()?;
    let configured_websocket_url: Option<String> = read_configured_websocket_url(&window).await?;
    let page_origin: PageOrigin = get_page_origin(&window)?;

    Ok(connection::resolve_websocket_url(
        configured_websocket_url.as_deref(),
        &page_origin,
    ))
}

async fn read_configured_websocket_url(window: &Window) -> Result<Option<String>, AppError> {
    let response_value: JsValue = JsFuture::from(window.fetch_with_str(GAME_SERVER_CONFIG_PATH))
        .await
        .map_err(|error| get_js_error("fetching the game server config failed", &error))?;
    let response: Response = response_value
        .dyn_into::<Response>()
        .map_err(|error| get_js_error("the game server config fetch gave no response", &error))?;

    if !response.ok() {
        return Err(AppError::new(&format!(
            "the game server config is unavailable; [status={}]",
            response.status(),
        )));
    }

    let text_promise: Promise =
        response.text().map_err(|error| get_js_error("reading the game server config failed", &error))?;
    let text_value: JsValue = JsFuture::from(text_promise)
        .await
        .map_err(|error| get_js_error("reading the game server config failed", &error))?;
    let text: String = text_value.as_string().unwrap_or_default();
    let config: JsValue =
        JSON::parse(&text).map_err(|error| get_js_error("the game server config is not JSON", &error))?;
    let websocket_url_value: JsValue = Reflect::get(&config, &JsValue::from_str(WEBSOCKET_URL_FIELD))
        .map_err(|error| get_js_error("the game server config is not a JSON object", &error))?;

    if websocket_url_value.is_undefined() {
        return Err(AppError::new("the game server config has no websocket_url"));
    }

    Ok(websocket_url_value.as_string())
}

fn get_page_origin(window: &Window) -> Result<PageOrigin, AppError> {
    let protocol: String = window
        .location()
        .protocol()
        .map_err(|error| get_js_error("reading the page protocol failed", &error))?;
    let host: String =
        window.location().host().map_err(|error| get_js_error("reading the page host failed", &error))?;

    Ok(PageOrigin { protocol, host })
}

fn get_window() -> Result<Window, AppError> {
    web_sys::window().ok_or_else(|| AppError::new("the page has no window"))
}

fn get_js_error(message: &str, error: &JsValue) -> AppError {
    AppError::new(&format!("{message}; [error={error:?}]"))
}
