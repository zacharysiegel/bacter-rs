use js_sys::{ArrayBuffer, Uint8Array};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{BinaryType, CloseEvent, Event, MessageEvent, WebSocket};

use shared::error::AppError;

const OPEN_EVENT: &str = "open";
const MESSAGE_EVENT: &str = "message";
const CLOSE_EVENT: &str = "close";

/// A binary WebSocket whose handlers live as long as it does.
pub struct GameSocket {
    websocket: WebSocket,
    open_listener: Closure<dyn FnMut(Event)>,
    message_listener: Closure<dyn FnMut(MessageEvent)>,
    close_listener: Closure<dyn FnMut(CloseEvent)>,
}

impl GameSocket {
    /// A failed connection reports itself through `on_close` with code 1006.
    pub fn open(
        url: &str,
        mut on_open: impl FnMut() + 'static,
        mut on_message: impl FnMut(Vec<u8>) + 'static,
        mut on_close: impl FnMut(u16) + 'static,
    ) -> Result<GameSocket, AppError> {
        let websocket: WebSocket = WebSocket::new(url)
            .map_err(|error| AppError::new(&format!("opening the game socket failed; [url={url} error={error:?}]")))?;
        websocket.set_binary_type(BinaryType::Arraybuffer);

        let open_listener: Closure<dyn FnMut(Event)> = Closure::new(move |_event: Event| on_open());
        let message_listener: Closure<dyn FnMut(MessageEvent)> = Closure::new(move |event: MessageEvent| {
            let frame: Option<Vec<u8>> = get_binary_frame(&event.data());

            match frame {
                Some(frame) => on_message(frame),
                None => log::warn!("text frame from the game server dropped"),
            }
        });
        let close_listener: Closure<dyn FnMut(CloseEvent)> =
            Closure::new(move |event: CloseEvent| on_close(event.code()));

        let game_socket: GameSocket = GameSocket {
            websocket,
            open_listener,
            message_listener,
            close_listener,
        };
        game_socket.add_listeners()?;

        Ok(game_socket)
    }

    pub fn send(&self, frame: &[u8]) {
        let sent: Result<(), JsValue> = self.websocket.send_with_u8_array(frame);

        if let Err(error) = sent {
            log::warn!("sending to the game server failed; [error={error:?}]");
        }
    }

    fn get_listeners(&self) -> [(&'static str, &JsValue); 3] {
        [
            (OPEN_EVENT, self.open_listener.as_ref()),
            (MESSAGE_EVENT, self.message_listener.as_ref()),
            (CLOSE_EVENT, self.close_listener.as_ref()),
        ]
    }

    fn add_listeners(&self) -> Result<(), AppError> {
        for (event_type, listener) in self.get_listeners() {
            self.websocket
                .add_event_listener_with_callback(event_type, listener.unchecked_ref())
                .map_err(|error| AppError::new(&format!("listening on the game socket failed; [error={error:?}]")))?;
        }

        Ok(())
    }
}

impl Drop for GameSocket {
    /// Detaches the handlers before they are freed; closing an already closed socket does nothing.
    fn drop(&mut self) {
        for (event_type, listener) in self.get_listeners() {
            let _ = self.websocket.remove_event_listener_with_callback(event_type, listener.unchecked_ref());
        }

        let _ = self.websocket.close();
    }
}

fn get_binary_frame(data: &JsValue) -> Option<Vec<u8>> {
    let array_buffer: &ArrayBuffer = data.dyn_ref::<ArrayBuffer>()?;

    Some(Uint8Array::new(array_buffer).to_vec())
}
