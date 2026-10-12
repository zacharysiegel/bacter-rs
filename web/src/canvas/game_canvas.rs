use shared::protocol::MessageSerialIn;

use crate::app::ClientSettings;
#[cfg(feature = "hydrate")] // the ssr build has no driver
use crate::canvas::driver;

/// What the views ask of the driver.
#[derive(Clone, Debug, PartialEq)]
pub enum DriverCommandKind {
    SendRequest(MessageSerialIn),
    StartTutorial,
    LeaveTutorial,
    Reconnect,
    StoreClientSettings(ClientSettings),
}

#[cfg(feature = "hydrate")] // the ssr build has no driver
pub fn dispatch_driver_command(command: DriverCommandKind) {
    driver::dispatch(command);
}

#[cfg(not(feature = "hydrate"))] // the hydrate build dispatches to the driver
pub fn dispatch_driver_command(_command: DriverCommandKind) {}
