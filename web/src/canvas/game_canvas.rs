use shared::protocol::MessageSerialIn;

use crate::app::ClientSettings;

/// What the views ask of the driver, which owns the socket and the scenes.
#[derive(Clone, Debug, PartialEq)]
pub enum DriverCommandKind {
    SendRequest(MessageSerialIn),
    StartTutorial,
    LeaveTutorial,
    Reconnect,
    StoreClientSettings(ClientSettings),
}

#[cfg(feature = "hydrate")] // the ssr build has no driver to command
pub use browser::*;
#[cfg(not(feature = "hydrate"))] // the ssr build has no driver
pub use server::*;

#[cfg(feature = "hydrate")] // the ssr build has no driver to command
mod browser {
    use crate::canvas::DriverCommandKind;
    use crate::canvas::driver;

    pub fn dispatch_driver_command(command: DriverCommandKind) {
        driver::dispatch(command);
    }
}

#[cfg(not(feature = "hydrate"))] // the ssr build renders the canvas without driving it
mod server {
    use crate::canvas::DriverCommandKind;

    pub fn dispatch_driver_command(_command: DriverCommandKind) {}
}
