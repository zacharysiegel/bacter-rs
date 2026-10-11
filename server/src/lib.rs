//! The game server.

pub mod config;
pub mod game;
pub mod lobby;
pub mod password;
pub mod registries;
pub mod shutdown;
pub mod site;
pub mod socket;

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use actix_web::dev::{Server, ServerHandle};
use actix_web::{App, HttpServer, web};
use tokio::sync::watch;
use tokio::task::{JoinError, JoinHandle};

use shared::error::AppError;

use crate::config::ServerConfig;
use crate::config::tls;
use crate::game::GameTaskConfig;
use crate::game::game_runtime;
use crate::lobby::lobby_broadcast;
use crate::registries::ServerRegistries;
use crate::shutdown::ServerRunState;
use crate::site::site_api;
use crate::socket::SocketContext;
use crate::socket::socket_api;

/// Lets the connections write their close frames before the server stops.
const SHUTDOWN_CLOSE_DELAY: Duration = Duration::from_millis(500);

/// A bound server which runs once awaited.
pub struct CreatedServer {
    pub server: Server,
    pub local_addresses: Vec<SocketAddr>,
    /// `ShuttingDown` closes every connection with `ServerShutdown`.
    pub run_state: watch::Sender<ServerRunState>,
}

pub fn create_server(config: ServerConfig, registries: ServerRegistries) -> Result<CreatedServer, AppError> {
    let (run_state_sender, run_state_receiver): (watch::Sender<ServerRunState>, watch::Receiver<ServerRunState>) =
        watch::channel(ServerRunState::Running);
    let socket_context: web::Data<SocketContext> = web::Data::new(SocketContext {
        registries,
        allowed_origins: config.allowed_origins.clone(),
        game_config: GameTaskConfig::from_server_config(&config),
        run_state: run_state_receiver,
    });
    let site_root: Option<PathBuf> = config.site_root.clone();

    let http_server = HttpServer::new(move || {
        let site_root: Option<PathBuf> = site_root.clone();

        App::new()
            .app_data(socket_context.clone())
            .configure(socket_api::configurer)
            .configure(move |service_config| site_api::configurer(service_config, site_root.as_deref()))
    })
    .disable_signals();

    let bind_address: (IpAddr, u16) = (config.bind_address, config.port);
    let bound_server = match &config.tls_files {
        Some(tls_files) => http_server.bind_rustls_0_23(bind_address, tls::create_rustls_config(tls_files)?)?,
        None => http_server.bind(bind_address)?,
    };
    let local_addresses: Vec<SocketAddr> = bound_server.addrs();

    game_runtime::GAME_RUNTIME.spawn(lobby_broadcast::run_lobby_task(registries));

    Ok(CreatedServer {
        server: bound_server.run(),
        local_addresses,
        run_state: run_state_sender,
    })
}

/// Until an interrupt or terminate signal, which closes every connection first.
pub async fn run(config: ServerConfig, registries: ServerRegistries) -> Result<(), AppError> {
    let created_server: CreatedServer = create_server(config, registries)?;
    let server_handle: ServerHandle = created_server.server.handle();
    let mut server_task: JoinHandle<io::Result<()>> = actix_web::rt::spawn(created_server.server);

    log::info!("server listening; [addresses={:?}]", created_server.local_addresses);

    tokio::select! {
        server_result = &mut server_task => return get_server_result(server_result),
        _ = shutdown::wait_for_termination_signal() => {}
    }

    log::info!("server shutting down");
    let _ = created_server.run_state.send(ServerRunState::ShuttingDown);
    tokio::time::sleep(SHUTDOWN_CLOSE_DELAY).await;
    server_handle.stop(true).await;

    get_server_result(server_task.await)
}

fn get_server_result(server_result: Result<io::Result<()>, JoinError>) -> Result<(), AppError> {
    let io_result: io::Result<()> =
        server_result.map_err(|error| AppError::from_error("the server task failed", Box::new(error)))?;

    Ok(io_result?)
}
