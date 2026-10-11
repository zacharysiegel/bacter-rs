#[cfg(not(unix))] // unix targets also wait for SIGTERM
pub use other::wait_for_termination_signal;
#[cfg(unix)] // other targets have no SIGTERM
pub use unix::wait_for_termination_signal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerRunState {
    Running,
    ShuttingDown,
}

#[cfg(unix)] // other targets have no SIGTERM
mod unix {
    use std::io;

    use actix_web::rt::signal;
    use actix_web::rt::signal::unix::{Signal, SignalKind};

    /// Interrupt or terminate.
    pub async fn wait_for_termination_signal() {
        let terminate_signal: io::Result<Signal> = signal::unix::signal(SignalKind::terminate());
        let mut terminate_signal: Signal = match terminate_signal {
            Ok(terminate_signal) => terminate_signal,
            Err(error) => {
                log::warn!("the SIGTERM handler could not be installed; waiting for ctrl-c only; [error={error}]");

                let _ = signal::ctrl_c().await;

                return;
            }
        };

        tokio::select! {
            _ = signal::ctrl_c() => {}
            _ = terminate_signal.recv() => {}
        }
    }
}

#[cfg(not(unix))] // unix targets also wait for SIGTERM
mod other {
    use actix_web::rt::signal;

    pub async fn wait_for_termination_signal() {
        let _ = signal::ctrl_c().await;
    }
}
