use std::net::IpAddr;
use std::time::{Duration, Instant};

use actix_ws::{Message, MessageStream, ProtocolError};
use bytes::Bytes;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::watch::error::RecvError;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::Interval;

use shared::error::AppError;
use shared::game::GameId;
use shared::member::MemberId;
use shared::protocol;
use shared::protocol::{CloseReasonKind, MessageSerialIn, MessageSerialOut};

use crate::game::game_handle;
use crate::game::{GameHandle, GameRequest, GameTaskConfig};
use crate::lobby::lobby_broadcast;
use crate::registries::ServerRegistries;
use crate::shutdown::ServerRunState;
use crate::socket::{
    CloseCauseKind, ConnectionControl, ConnectionId, ConnectionPhase, ProtocolViolationKind, TokenBucket, WriterCommand,
};
use crate::socket::{connection_limit, connection_model, request_dispatch};

pub const PING_PERIOD: Duration = Duration::from_secs(10);
pub const INBOUND_TIMEOUT: Duration = Duration::from_secs(30);

/// The state of one socket, owned by its read task.
pub struct Connection {
    pub connection_id: ConnectionId,
    pub peer_ip: IpAddr,
    pub registries: ServerRegistries,
    pub game_config: GameTaskConfig,
    /// None in the lobby.
    pub joined_game: Option<JoinedGame>,
    pub writer: mpsc::Sender<WriterCommand>,
    pub lobby_sender: mpsc::Sender<Bytes>,
    pub input_bucket: TokenBucket,
    pub request_bucket: TokenBucket,
    pub password_failure_count: u32,
    pub dropped_input_count: u64,
    last_inbound_at: Instant,
}

impl Connection {
    pub fn new(
        connection_id: ConnectionId,
        peer_ip: IpAddr,
        registries: ServerRegistries,
        game_config: GameTaskConfig,
        writer: mpsc::Sender<WriterCommand>,
        lobby_sender: mpsc::Sender<Bytes>,
    ) -> Connection {
        let now: Instant = Instant::now();

        Connection {
            connection_id,
            peer_ip,
            registries,
            game_config,
            joined_game: None,
            writer,
            lobby_sender,
            input_bucket: TokenBucket::create_input_bucket(now),
            request_bucket: TokenBucket::create_request_bucket(now),
            password_failure_count: 0,
            dropped_input_count: 0,
            last_inbound_at: now,
        }
    }

    pub fn enter_game(
        &mut self,
        game_id: GameId,
        member_id: MemberId,
        handle: GameHandle,
        control: watch::Receiver<ConnectionControl>,
    ) {
        self.joined_game = Some(JoinedGame {
            game_id,
            member_id,
            handle,
            control,
        });
    }

    pub fn phase(&self) -> ConnectionPhase {
        match &self.joined_game {
            Some(joined_game) => ConnectionPhase::InGame {
                game_id: joined_game.game_id,
                member_id: joined_game.member_id,
            },
            None => ConnectionPhase::Lobby,
        }
    }

    pub fn return_to_lobby(&mut self) {
        self.joined_game = None;
    }

    /// `Finish` when the writer is gone.
    pub async fn send_to_writer(&self, command: WriterCommand) -> ConnectionFlow {
        let send_result: Result<(), SendError<WriterCommand>> = self.writer.send(command).await;

        match send_result {
            Ok(()) => ConnectionFlow::Continue,
            Err(_) => ConnectionFlow::Finish,
        }
    }

    pub async fn send_message(&self, message_serial_out: &MessageSerialOut) -> ConnectionFlow {
        self.send_to_writer(WriterCommand::Frame(connection_model::encode_frame(message_serial_out))).await
    }
}

pub struct JoinedGame {
    pub game_id: GameId,
    pub member_id: MemberId,
    pub handle: GameHandle,
    pub control: watch::Receiver<ConnectionControl>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionFlow {
    Continue,
    Close(CloseCauseKind),
    /// Ends without a close frame, which could not be written.
    Finish,
}

enum ConnectionEventKind {
    Inbound(Option<Result<Message, ProtocolError>>),
    ControlChanged(Result<ConnectionControl, RecvError>),
    WriterEnded,
    PingDue,
    RunStateChanged(Result<ServerRunState, RecvError>),
}

/// Owns the connection state and never writes to the socket itself.
pub async fn run_connection(
    mut connection: Connection,
    mut stream: MessageStream,
    mut writer_handle: JoinHandle<()>,
    mut run_state: watch::Receiver<ServerRunState>,
) {
    let mut ping_interval: Interval = tokio::time::interval_at(tokio::time::Instant::now() + PING_PERIOD, PING_PERIOD);
    let mut is_run_state_watched: bool = true;

    loop {
        let event: ConnectionEventKind = tokio::select! {
            message = stream.recv() => ConnectionEventKind::Inbound(message),
            control = wait_for_control_change(&mut connection.joined_game) => ConnectionEventKind::ControlChanged(control),
            _ = &mut writer_handle => ConnectionEventKind::WriterEnded,
            _ = ping_interval.tick() => ConnectionEventKind::PingDue,
            run_state = wait_for_run_state_change(&mut run_state), if is_run_state_watched => ConnectionEventKind::RunStateChanged(run_state),
        };

        let flow: ConnectionFlow = match event {
            ConnectionEventKind::Inbound(message) => handle_inbound(&mut connection, message).await,
            ConnectionEventKind::ControlChanged(control) => handle_control_change(&mut connection, control).await,
            ConnectionEventKind::WriterEnded => ConnectionFlow::Finish,
            ConnectionEventKind::PingDue => handle_ping_due(&connection).await,
            ConnectionEventKind::RunStateChanged(Ok(ServerRunState::ShuttingDown)) => {
                ConnectionFlow::Close(CloseCauseKind::Reason(CloseReasonKind::ServerShutdown))
            }
            ConnectionEventKind::RunStateChanged(Ok(ServerRunState::Running)) => ConnectionFlow::Continue,
            ConnectionEventKind::RunStateChanged(Err(_)) => {
                is_run_state_watched = false;

                ConnectionFlow::Continue
            }
        };

        match flow {
            ConnectionFlow::Continue => {}
            ConnectionFlow::Close(cause) => {
                connection.send_to_writer(WriterCommand::Close(cause)).await;

                break;
            }
            ConnectionFlow::Finish => break,
        }
    }

    finish_connection(&connection).await;
}

async fn wait_for_run_state_change(
    run_state: &mut watch::Receiver<ServerRunState>,
) -> Result<ServerRunState, RecvError> {
    run_state.changed().await?;

    Ok(*run_state.borrow_and_update())
}

/// Never finishes outside a game.
async fn wait_for_control_change(joined_game: &mut Option<JoinedGame>) -> Result<ConnectionControl, RecvError> {
    let Some(joined_game) = joined_game else {
        return std::future::pending().await;
    };

    joined_game.control.changed().await?;

    Ok(*joined_game.control.borrow_and_update())
}

async fn handle_inbound(
    connection: &mut Connection,
    message: Option<Result<Message, ProtocolError>>,
) -> ConnectionFlow {
    let message: Message = match message {
        None | Some(Err(ProtocolError::Io(_))) => return ConnectionFlow::Finish,
        Some(Err(ProtocolError::Overflow)) => return close_for_violation(ProtocolViolationKind::Oversize),
        Some(Err(_)) => return close_for_violation(ProtocolViolationKind::Decode),
        Some(Ok(message)) => message,
    };

    connection.last_inbound_at = Instant::now();

    match message {
        Message::Binary(frame) => handle_binary_frame(connection, &frame).await,
        Message::Text(_) => close_for_violation(ProtocolViolationKind::TextFrame),
        Message::Continuation(_) => close_for_violation(ProtocolViolationKind::Decode),
        Message::Ping(payload) => connection.send_to_writer(WriterCommand::Pong(payload)).await,
        Message::Pong(_) | Message::Nop => ConnectionFlow::Continue,
        Message::Close(_) => ConnectionFlow::Close(CloseCauseKind::ClientClosed),
    }
}

async fn handle_binary_frame(connection: &mut Connection, frame: &[u8]) -> ConnectionFlow {
    let decode_result: Result<MessageSerialIn, AppError> = protocol::decode_message_in(frame);

    match decode_result {
        Ok(message_serial_in) => request_dispatch::dispatch(connection, message_serial_in).await,
        Err(_) => close_for_violation(ProtocolViolationKind::Decode),
    }
}

fn close_for_violation(violation: ProtocolViolationKind) -> ConnectionFlow {
    ConnectionFlow::Close(CloseCauseKind::ProtocolViolation(violation))
}

/// A game which ended without closing the connection sends the member back to the lobby.
async fn handle_control_change(
    connection: &mut Connection,
    control: Result<ConnectionControl, RecvError>,
) -> ConnectionFlow {
    match control {
        Ok(ConnectionControl::Close(cause)) => ConnectionFlow::Close(cause),
        Ok(ConnectionControl::Open) => ConnectionFlow::Continue,
        Err(_) => {
            connection.return_to_lobby();

            let game_ended_frame: Bytes = connection_model::encode_frame(&MessageSerialOut::GameEnded);

            connection
                .send_to_writer(WriterCommand::DetachMembership {
                    then_send: Some(game_ended_frame),
                })
                .await
        }
    }
}

async fn handle_ping_due(connection: &Connection) -> ConnectionFlow {
    let is_inbound_timed_out: bool = connection.last_inbound_at.elapsed() >= INBOUND_TIMEOUT;

    if is_inbound_timed_out {
        return ConnectionFlow::Close(CloseCauseKind::Reason(CloseReasonKind::InboundTimeout));
    }

    connection.send_to_writer(WriterCommand::Ping).await
}

async fn finish_connection(connection: &Connection) {
    if let Some(joined_game) = &connection.joined_game {
        let connection_closed: GameRequest = GameRequest::ConnectionClosed {
            connection_id: connection.connection_id,
        };
        let _ = joined_game.handle.requests.send_timeout(connection_closed, game_handle::REQUEST_SEND_TIMEOUT).await;
    }

    lobby_broadcast::unsubscribe(connection.registries, connection.connection_id);
    connection_limit::release_connection(connection.registries, connection.peer_ip);

    log::debug!(
        "connection closed; [connection_id={} dropped_inputs={}]",
        connection.connection_id.0,
        connection.dropped_input_count,
    );
}
