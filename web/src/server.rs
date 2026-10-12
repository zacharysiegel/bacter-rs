use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use any_spawner::Executor;
use axum::Router;
use axum::body::{Body, Bytes};
use axum::http::{Request, StatusCode};
use axum::response::Response;
use axum::routing;
use leptos::prelude::*;
use tokio::net::TcpListener;

use shared::error::AppError;

use crate::app;

/// The one route; the screen is client state, never a URL.
pub const PRERENDERED_ROUTE_PATH: &str = "/";
const PRERENDERED_DOCUMENT_NAME: &str = "index.html";
const CRATE_DIRECTORY: &str = env!("CARGO_MANIFEST_DIR");
const MANIFEST_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

/// `cargo leptos watch`'s server: the shell at `/`, the site root's files elsewhere.
pub async fn serve() -> Result<(), AppError> {
    initialize_executor();

    let configuration: ConfFile = get_configuration(None)
        .map_err(|error| AppError::from_error("reading the leptos settings failed", Box::new(error)))?;
    let leptos_options: LeptosOptions = configuration.leptos_options;
    let address: SocketAddr = leptos_options.site_addr;
    let shell_options: LeptosOptions = leptos_options.clone();
    let router: Router = Router::new()
        .route(
            PRERENDERED_ROUTE_PATH,
            routing::get(leptos_axum::render_app_to_stream(move || {
                app::shell(shell_options.clone())
            })),
        )
        .fallback(leptos_axum::file_and_error_handler(app::shell))
        .with_state(leptos_options);
    let listener: TcpListener = TcpListener::bind(&address).await.map_err(|error| {
        AppError::from_error(
            &format!("binding the dev server failed; [address={address}]"),
            Box::new(error),
        )
    })?;

    leptos::logging::log!("listening; [address=http://{address}]");

    axum::serve(listener, router.into_make_service())
        .await
        .map_err(|error| AppError::from_error("the dev server stopped", Box::new(error)))
}

/// Writes `index.html` into the site root of a release build.
pub async fn write_prerendered_document() -> Result<(), AppError> {
    let leptos_options: LeptosOptions = read_leptos_manifest_options()?;
    let site_root: PathBuf = resolve_site_root(&leptos_options)?;
    let document_bytes: Bytes = prerender_document(leptos_options).await?;
    let document_path: PathBuf = site_root.join(PRERENDERED_DOCUMENT_NAME);

    fs::write(&document_path, &document_bytes).map_err(|error| {
        AppError::from_error(
            &format!(
                "writing the prerendered document failed; [path={}]",
                document_path.display(),
            ),
            Box::new(error),
        )
    })?;

    println!(
        "wrote the prerendered document; [path={} bytes={}]",
        document_path.display(),
        document_bytes.len(),
    );

    Ok(())
}

/// One complete document of the `/` route.
pub async fn prerender_document(leptos_options: LeptosOptions) -> Result<Bytes, AppError> {
    initialize_executor();

    let render_document = leptos_axum::render_app_async(move || app::shell(leptos_options.clone()));
    let request: Request<Body> = Request::builder()
        .uri(PRERENDERED_ROUTE_PATH)
        .body(Body::empty())
        .map_err(|error| AppError::from_error("building the render request failed", Box::new(error)))?;
    let response: Response = render_document(request).await;
    let status: StatusCode = response.status();

    if !status.is_success() {
        return Err(AppError::new(&format!(
            "rendering the document did not succeed; [status={status} path={PRERENDERED_ROUTE_PATH}]",
        )));
    }

    axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|error| AppError::from_error("reading the rendered document failed", Box::new(error)))
}

/// cargo-leptos sets the `LEPTOS_*` variables only for the processes it runs, so a prerender reads the manifest.
pub fn read_leptos_manifest_options() -> Result<LeptosOptions, AppError> {
    let configuration: ConfFile = get_configuration(Some(MANIFEST_PATH)).map_err(|error| {
        AppError::from_error(
            &format!("reading the leptos settings failed; [path={MANIFEST_PATH}]"),
            Box::new(error),
        )
    })?;

    Ok(configuration.leptos_options)
}

/// Rendering spawns onto a global executor; an error means one is already set.
fn initialize_executor() {
    let _ = Executor::init_tokio();
}

/// `site-root` is relative to the workspace root, the web crate's parent.
fn resolve_site_root(leptos_options: &LeptosOptions) -> Result<PathBuf, AppError> {
    let workspace_root: &Path = Path::new(CRATE_DIRECTORY)
        .parent()
        .ok_or_else(|| AppError::new("the web crate has no parent directory"))?;
    let declared_site_root: PathBuf = workspace_root.join(leptos_options.site_root.as_ref());

    fs::canonicalize(&declared_site_root).map_err(|error| {
        AppError::from_error(
            &format!(
                "the site root does not exist, so no build has run; [path={}]",
                declared_site_root.display(),
            ),
            Box::new(error),
        )
    })
}
