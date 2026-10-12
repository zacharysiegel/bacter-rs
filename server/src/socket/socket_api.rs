use std::net::{IpAddr, Ipv4Addr};

use actix_web::http::header;
use actix_web::{HttpRequest, HttpResponse, rt, web};
use actix_ws::{MessageStream, Session};
use bytes::Bytes;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use shared::protocol;
use shared::protocol::{CloseReasonKind, protocol_limits};

use crate::config;
use crate::game::GameTaskConfig;
use crate::lobby::lobby_broadcast;
use crate::registries::ServerRegistries;
use crate::shutdown::ServerRunState;
use crate::socket::{CloseCauseKind, Connection, ConnectionId, ConnectionLimitReached, WriterCommand};
use crate::socket::{connection, connection_limit, connection_model, socket_writer};

/// Shared by every connection of one server.
pub struct SocketContext {
    pub registries: ServerRegistries,
    pub allowed_origins: Vec<String>,
    pub game_config: GameTaskConfig,
    pub run_state: watch::Receiver<ServerRunState>,
}

pub fn configurer(service_config: &mut web::ServiceConfig) {
    service_config.route("/ws", web::get().to(open_socket));
}

/// Version and connection-limit failures upgrade, then close at once, so the client can read the close reason.
async fn open_socket(
    request: HttpRequest,
    body: web::Payload,
    context: web::Data<SocketContext>,
) -> Result<HttpResponse, actix_web::Error> {
    let origin: Option<&str> = request.headers().get(header::ORIGIN).and_then(|origin| origin.to_str().ok());
    let is_origin_allowed: bool =
        origin.is_some_and(|origin| config::is_origin_allowed(&context.allowed_origins, origin));

    if !is_origin_allowed {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let (response, session, stream): (HttpResponse, Session, MessageStream) = actix_ws::handle(&request, body)?;
    let peer_ip: IpAddr =
        request.peer_addr().map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |peer_address| peer_address.ip());
    let protocol_version: Option<u16> = get_protocol_version(request.query_string());

    if protocol_version != Some(protocol::PROTOCOL_VERSION) {
        close_after_upgrade(session, CloseReasonKind::ProtocolVersionMismatch);

        return Ok(response);
    }

    let reservation: Result<(), ConnectionLimitReached> =
        connection_limit::reserve_connection(context.registries, peer_ip);

    if reservation.is_err() {
        close_after_upgrade(session, CloseReasonKind::ServerFull);

        return Ok(response);
    }

    start_connection(&context, session, stream, peer_ip);

    Ok(response)
}

fn close_after_upgrade(session: Session, reason: CloseReasonKind) {
    let cause: CloseCauseKind = CloseCauseKind::Reason(reason);

    rt::spawn(socket_writer::close_session(session, cause));
}

fn get_protocol_version(query_string: &str) -> Option<u16> {
    query_string
        .split('&')
        .filter_map(|parameter| parameter.split_once('='))
        .find(|(name, _)| *name == protocol::PROTOCOL_VERSION_QUERY_PARAMETER)
        .and_then(|(_, value)| value.parse::<u16>().ok())
}

/// The read task and the writer task, both on the accepting worker.
fn start_connection(context: &SocketContext, session: Session, stream: MessageStream, peer_ip: IpAddr) {
    let connection_id: ConnectionId = connection_model::allocate_connection_id(context.registries);
    let (writer_sender, writer_receiver): (mpsc::Sender<WriterCommand>, mpsc::Receiver<WriterCommand>) =
        mpsc::channel(socket_writer::WRITER_COMMAND_CHANNEL_CAPACITY);
    let (lobby_sender, lobby_receiver): (mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) =
        mpsc::channel(lobby_broadcast::LOBBY_CHANNEL_CAPACITY);
    let stream: MessageStream = stream.max_frame_size(protocol_limits::INBOUND_FRAME_BYTE_LIMIT);

    let connection: Connection = Connection::new(
        connection_id,
        peer_ip,
        context.registries,
        context.game_config.clone(),
        writer_sender,
        lobby_sender,
    );
    let writer_handle: JoinHandle<()> = rt::spawn(socket_writer::run_socket_writer(
        session,
        writer_receiver,
        lobby_receiver,
    ));

    rt::spawn(connection::run_connection(
        connection,
        stream,
        writer_handle,
        context.run_state.clone(),
    ));

    log::debug!(
        "connection opened; [connection_id={} peer_ip={peer_ip}]",
        connection_id.0
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_protocol_version_reads_the_query_parameter() {
        assert_eq!(get_protocol_version("protocol_version=1"), Some(1));
        assert_eq!(get_protocol_version("other=3&protocol_version=12"), Some(12));
        assert_eq!(get_protocol_version("protocol_version=x"), None);
        assert_eq!(get_protocol_version("version=1"), None);
        assert_eq!(get_protocol_version(""), None);
    }
}
