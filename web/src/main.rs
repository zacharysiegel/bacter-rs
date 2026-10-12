// The server binary resolves the same nested view! types as the lib (see lib.rs), past rustc's default limit of 128.
#![recursion_limit = "512"]

#[cfg(feature = "ssr")] // a browser build has no server to start
use shared::error::AppError;

#[cfg(feature = "ssr")] // a browser build has no server to start
const PRERENDER_ARGUMENT: &str = "prerender";

#[cfg(feature = "ssr")] // a browser build has no process to exit
enum ExitStatusKind {
    Failed = 1,
    Usage = 64,
}

#[cfg(feature = "ssr")] // a browser build has no server to start
#[tokio::main]
async fn main() {
    let first_argument: Option<String> = std::env::args().nth(1);

    let result: Result<(), AppError> = match first_argument.as_deref() {
        None => web::server::serve().await,
        Some(PRERENDER_ARGUMENT) => web::server::write_prerendered_document().await,
        Some(unrecognized_argument) => {
            eprintln!("unrecognized argument; [argument={unrecognized_argument} expected={PRERENDER_ARGUMENT}]");
            std::process::exit(ExitStatusKind::Usage as i32);
        }
    };

    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(ExitStatusKind::Failed as i32);
    }
}

#[cfg(not(feature = "ssr"))] // the browser build's entry point is the lib's hydrate function
fn main() {}
