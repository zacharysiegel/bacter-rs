use leptos::html::Canvas;
use leptos::prelude::*;

use shared::protocol::MessageSerialIn;

use crate::app::{AppSignals, ClientSettings};

/// What the views ask of the driver, which owns the socket and the scenes.
#[derive(Clone, Debug, PartialEq)]
pub enum DriverCommandKind {
    SendRequest(MessageSerialIn),
    StartTutorial,
    LeaveTutorial,
    Reconnect,
    StoreClientSettings(ClientSettings),
}

/// Behind every other layer.
#[component]
pub fn GameCanvas() -> impl IntoView {
    let signals: AppSignals = expect_context();
    let canvas_ref: NodeRef<Canvas> = NodeRef::new();

    start_driver_when_mounted(canvas_ref, signals);

    view! { <canvas node_ref=canvas_ref class="game-canvas"></canvas> }
}

#[cfg(feature = "hydrate")] // the ssr build has no driver to command
pub use browser::*;
#[cfg(not(feature = "hydrate"))] // the ssr build has no driver
pub use server::*;

#[cfg(feature = "hydrate")] // the ssr build has no driver to command
mod browser {
    use leptos::html::Canvas;
    use leptos::prelude::*;

    use crate::app::AppSignals;
    use crate::canvas::DriverCommandKind;
    use crate::canvas::driver;

    pub fn start_driver_when_mounted(canvas_ref: NodeRef<Canvas>, signals: AppSignals) {
        Effect::new(move |_| {
            if let Some(canvas) = canvas_ref.get() {
                driver::start(canvas, signals);
            }
        });
    }

    pub fn dispatch_driver_command(command: DriverCommandKind) {
        driver::dispatch(command);
    }
}

#[cfg(not(feature = "hydrate"))] // the ssr build renders the canvas without driving it
mod server {
    use leptos::html::Canvas;
    use leptos::prelude::*;

    use crate::app::AppSignals;
    use crate::canvas::DriverCommandKind;

    pub fn start_driver_when_mounted(_canvas_ref: NodeRef<Canvas>, _signals: AppSignals) {}

    pub fn dispatch_driver_command(_command: DriverCommandKind) {}
}
