use std::cell::RefCell;
use std::collections::VecDeque;

use crate::canvas::DriverCommandKind;

thread_local! {
    /// Commands wait here while the driver is busy or not yet started.
    static PENDING_DRIVER_COMMANDS: RefCell<VecDeque<DriverCommandKind>> = const { RefCell::new(VecDeque::new()) };
}

pub fn dispatch(command: DriverCommandKind) {
    PENDING_DRIVER_COMMANDS.with_borrow_mut(|pending_commands| pending_commands.push_back(command));
}
