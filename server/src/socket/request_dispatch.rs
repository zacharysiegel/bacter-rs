use std::time::Instant;

use bytes::Bytes;
use tokio::sync::mpsc::error::{SendError, SendTimeoutError, TrySendError};
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;

use shared::ability::Loadout;
use shared::error::AppError;
use shared::game::{GameId, GameSettings, PlayerInput};
use shared::member::{Appearance, Joiner, MemberId, TeamChoiceKind};
use shared::protocol::protocol_limits;
use shared::protocol::{
    AppearanceSerial, CloseReasonKind, GameSettingsSerialIn, JoinerSerialIn, LoadoutSerial, MessageSerialIn,
    MessageSerialOut, PlayerInputSerialIn, RejectionKind, RejectionKindSerialOut, RequestKind, RequestKindSerialOut,
    TeamChoiceKindSerialIn,
};

use crate::game::{ConnectionEndpoint, GameHandle, GameInput, GameRequest, GameReservation};
use crate::game::{game_handle, game_registry, game_supervisor};
use crate::lobby::lobby_broadcast;
use crate::password;
use crate::password::password_failure_limit;
use crate::password::{PasswordAttemptKind, PasswordMatchKind};
use crate::socket::connection_model;
use crate::socket::{
    CloseCauseKind, Connection, ConnectionFlow, ConnectionPhase, Membership, ProtocolViolationKind, TokenTakeKind,
    WriterCommand,
};

/// Whether a request may proceed; a shut gate has already answered it.
enum RequestGate {
    Open,
    Shut(ConnectionFlow),
}

/// Whether an in-game request may proceed; a shut gate has already answered it.
enum GameRequestGate {
    Open {
        member_id: MemberId,
        game_handle: GameHandle,
    },
    Shut(ConnectionFlow),
}

enum JoinKind {
    Participant(Joiner),
    Spectator(String),
}

/// Request payloads which fail their conversion are answered `RequestRejected`; any other invalid message closes
/// the connection.
pub async fn dispatch(connection: &mut Connection, message_serial_in: MessageSerialIn) -> ConnectionFlow {
    match message_serial_in {
        MessageSerialIn::SubscribeGameList => subscribe_game_list(connection).await,
        MessageSerialIn::UnsubscribeGameList => {
            lobby_broadcast::unsubscribe(connection.registries, connection.connection_id);

            ConnectionFlow::Continue
        }
        MessageSerialIn::CreateGame {
            settings,
            password,
            joiner,
        } => create_game(connection, settings, password, joiner).await,
        MessageSerialIn::JoinGame {
            game_id,
            password,
            joiner,
        } => join_game(connection, GameId(game_id), password, joiner).await,
        MessageSerialIn::SpectateGame {
            game_id,
            password,
            screen_name,
        } => spectate_game(connection, GameId(game_id), password, screen_name).await,
        MessageSerialIn::Respawn { loadout, team } => respawn(connection, loadout, team).await,
        MessageSerialIn::UpdateAppearance { appearance } => update_appearance(connection, appearance).await,
        MessageSerialIn::Input(player_input_serial_in) => send_input(connection, player_input_serial_in),
        MessageSerialIn::RequestSnapshot { .. } => request_snapshot(connection).await,
        MessageSerialIn::LeaveGame => leave_game(connection).await,
    }
}

async fn subscribe_game_list(connection: &mut Connection) -> ConnectionFlow {
    let gate: RequestGate = open_lobby_request(connection, RequestKind::SubscribeGameList).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    lobby_broadcast::subscribe(
        connection.registries,
        connection.connection_id,
        connection.lobby_sender.clone(),
    );

    ConnectionFlow::Continue
}

async fn create_game(
    connection: &mut Connection,
    settings_serial_in: GameSettingsSerialIn,
    password: Option<String>,
    joiner_serial_in: JoinerSerialIn,
) -> ConnectionFlow {
    let gate: RequestGate = open_lobby_request(connection, RequestKind::CreateGame).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    let creation: Result<(GameSettings, Joiner, Option<String>), RejectionKind> =
        convert_creation(settings_serial_in, joiner_serial_in, password);
    let (settings, joiner, password): (GameSettings, Joiner, Option<String>) = match creation {
        Ok(creation) => creation,
        Err(rejection) => return reject(connection, RequestKind::CreateGame, rejection).await,
    };

    let reservation_result: Result<GameReservation, RejectionKind> =
        game_registry::reserve_game(connection.registries, &settings.title);
    let reservation: GameReservation = match reservation_result {
        Ok(reservation) => reservation,
        Err(rejection) => return reject(connection, RequestKind::CreateGame, rejection).await,
    };

    let launch: Result<GameHandle, RejectionKind> =
        launch_reserved_game(connection, &reservation, settings, password).await;
    let game_handle: GameHandle = match launch {
        Ok(game_handle) => game_handle,
        Err(rejection) => {
            game_registry::cancel_reservation(connection.registries, &reservation);

            return reject(connection, RequestKind::CreateGame, rejection).await;
        }
    };

    request_membership(
        connection,
        RequestKind::CreateGame,
        reservation.game_id,
        game_handle,
        JoinKind::Participant(joiner),
    )
    .await
}

fn convert_creation(
    settings_serial_in: GameSettingsSerialIn,
    joiner_serial_in: JoinerSerialIn,
    password: Option<String>,
) -> Result<(GameSettings, Joiner, Option<String>), RejectionKind> {
    let settings: GameSettings = GameSettings::try_from(settings_serial_in)?;
    let joiner: Joiner = Joiner::try_from(joiner_serial_in)?;
    let password: Option<String> = protocol_limits::normalize_password(password)?;

    Ok((settings, joiner, password))
}

async fn launch_reserved_game(
    connection: &Connection,
    reservation: &GameReservation,
    settings: GameSettings,
    password: Option<String>,
) -> Result<GameHandle, RejectionKind> {
    let password_hash: Option<String> = match password {
        Some(password) => Some(password::hash_password(password).await?),
        None => None,
    };

    let seed: u64 = getrandom::u64().map_err(|error| {
        log::error!(
            "cannot draw a game seed; [error={}]",
            AppError::from_error("getrandom", Box::new(error))
        );

        RejectionKind::ServerBusy
    })?;

    Ok(game_supervisor::launch_game(
        connection.registries,
        &connection.game_config,
        reservation,
        settings,
        seed,
        password_hash,
    ))
}

async fn join_game(
    connection: &mut Connection,
    game_id: GameId,
    password: Option<String>,
    joiner_serial_in: JoinerSerialIn,
) -> ConnectionFlow {
    let gate: RequestGate = open_lobby_request(connection, RequestKind::JoinGame).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    let joiner_conversion: Result<Joiner, RejectionKind> = Joiner::try_from(joiner_serial_in);
    let joiner: Joiner = match joiner_conversion {
        Ok(joiner) => joiner,
        Err(rejection) => return reject(connection, RequestKind::JoinGame, rejection).await,
    };

    enter_existing_game(
        connection,
        RequestKind::JoinGame,
        game_id,
        password,
        JoinKind::Participant(joiner),
    )
    .await
}

async fn spectate_game(
    connection: &mut Connection,
    game_id: GameId,
    password: Option<String>,
    screen_name: String,
) -> ConnectionFlow {
    let gate: RequestGate = open_lobby_request(connection, RequestKind::SpectateGame).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    let screen_name_normalization: Result<String, RejectionKind> = protocol_limits::normalize_screen_name(&screen_name);
    let screen_name: String = match screen_name_normalization {
        Ok(screen_name) => screen_name,
        Err(rejection) => return reject(connection, RequestKind::SpectateGame, rejection).await,
    };

    enter_existing_game(
        connection,
        RequestKind::SpectateGame,
        game_id,
        password,
        JoinKind::Spectator(screen_name),
    )
    .await
}

async fn enter_existing_game(
    connection: &mut Connection,
    request_kind: RequestKind,
    game_id: GameId,
    password: Option<String>,
    join_kind: JoinKind,
) -> ConnectionFlow {
    let password_normalization: Result<Option<String>, RejectionKind> = protocol_limits::normalize_password(password);
    let password: Option<String> = match password_normalization {
        Ok(password) => password,
        Err(rejection) => return reject(connection, request_kind, rejection).await,
    };

    let registered_game_handle: Option<GameHandle> = game_registry::get_game(connection.registries, game_id);
    let Some(game_handle) = registered_game_handle else {
        return reject(connection, request_kind, RejectionKind::GameNotFound).await;
    };

    let gate: RequestGate = check_game_password(connection, request_kind, &game_handle, password).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    request_membership(connection, request_kind, game_id, game_handle, join_kind).await
}

/// Reaching the failure limit on one connection closes it; an IP past its failure limit is refused without hashing.
async fn check_game_password(
    connection: &mut Connection,
    request_kind: RequestKind,
    game_handle: &GameHandle,
    password: Option<String>,
) -> RequestGate {
    let Some(password_hash) = game_handle.password_hash.clone() else {
        return RequestGate::Open;
    };

    let Some(password) = password else {
        return RequestGate::Shut(reject(connection, request_kind, RejectionKind::PasswordRequired).await);
    };

    let attempt: PasswordAttemptKind = password_failure_limit::check_attempt(
        connection.registries.password_failures_per_ip,
        connection.peer_ip,
        Instant::now(),
    );

    if attempt == PasswordAttemptKind::RateLimited {
        return RequestGate::Shut(reject(connection, request_kind, RejectionKind::RateLimited).await);
    }

    let verification: Result<PasswordMatchKind, RejectionKind> =
        password::verify_password(password, password_hash).await;

    match verification {
        Ok(PasswordMatchKind::Matches) => RequestGate::Open,
        Ok(PasswordMatchKind::DoesNotMatch) => {
            RequestGate::Shut(record_password_failure(connection, request_kind).await)
        }
        Err(rejection) => RequestGate::Shut(reject(connection, request_kind, rejection).await),
    }
}

async fn record_password_failure(connection: &mut Connection, request_kind: RequestKind) -> ConnectionFlow {
    password_failure_limit::record_failure(
        connection.registries.password_failures_per_ip,
        connection.peer_ip,
        Instant::now(),
    );
    connection.password_failure_count += 1;

    if connection.password_failure_count >= password::PASSWORD_FAILURE_LIMIT {
        let cause: CloseCauseKind = CloseCauseKind::Reason(CloseReasonKind::PasswordFailureLimit);

        return ConnectionFlow::Close(cause);
    }

    reject(connection, request_kind, RejectionKind::PasswordIncorrect).await
}

async fn request_membership(
    connection: &mut Connection,
    request_kind: RequestKind,
    game_id: GameId,
    game_handle: GameHandle,
    join_kind: JoinKind,
) -> ConnectionFlow {
    let (endpoint, membership): (ConnectionEndpoint, Membership) =
        connection_model::create_membership(connection.connection_id);
    let (reply_sender, reply_receiver): (
        oneshot::Sender<Result<MemberId, RejectionKind>>,
        oneshot::Receiver<Result<MemberId, RejectionKind>>,
    ) = oneshot::channel();
    let request: GameRequest = match join_kind {
        JoinKind::Participant(joiner) => GameRequest::Join {
            endpoint,
            joiner,
            reply: reply_sender,
        },
        JoinKind::Spectator(screen_name) => GameRequest::Spectate {
            endpoint,
            screen_name,
            reply: reply_sender,
        },
    };

    let admission: Result<MemberId, RejectionKind> = send_answered_request(&game_handle, request, reply_receiver).await;
    let member_id: MemberId = match admission {
        Ok(member_id) => member_id,
        Err(rejection) => return reject(connection, request_kind, rejection).await,
    };

    lobby_broadcast::unsubscribe(connection.registries, connection.connection_id);
    connection.enter_game(game_id, member_id, game_handle, membership.control);

    // Attached only after the game accepted; the frames it queued meanwhile wait in the queue.
    connection.send_to_writer(WriterCommand::AttachMembership(membership.outbound)).await
}

/// A dropped reply means the game is gone.
async fn send_answered_request<T>(
    game_handle: &GameHandle,
    request: GameRequest,
    reply_receiver: oneshot::Receiver<Result<T, RejectionKind>>,
) -> Result<T, RejectionKind> {
    send_request(game_handle, request).await?;

    let reply: Result<Result<T, RejectionKind>, RecvError> = reply_receiver.await;

    reply.unwrap_or(Err(RejectionKind::GameNotFound))
}

/// A full request queue answers `ServerBusy`; a closed one means the game is gone.
async fn send_request(game_handle: &GameHandle, request: GameRequest) -> Result<(), RejectionKind> {
    let send_result: Result<(), SendTimeoutError<GameRequest>> =
        game_handle.requests.send_timeout(request, game_handle::REQUEST_SEND_TIMEOUT).await;

    match send_result {
        Ok(()) => Ok(()),
        Err(SendTimeoutError::Timeout(_)) => Err(RejectionKind::ServerBusy),
        Err(SendTimeoutError::Closed(_)) => Err(RejectionKind::GameNotFound),
    }
}

async fn respawn(
    connection: &mut Connection,
    loadout_serial: LoadoutSerial,
    team_serial_in: TeamChoiceKindSerialIn,
) -> ConnectionFlow {
    let gate: GameRequestGate = open_game_request(connection, RequestKind::Respawn).await;
    let (member_id, game_handle): (MemberId, GameHandle) = match gate {
        GameRequestGate::Open { member_id, game_handle } => (member_id, game_handle),
        GameRequestGate::Shut(flow) => return flow,
    };

    let (reply_sender, reply_receiver): (
        oneshot::Sender<Result<(), RejectionKind>>,
        oneshot::Receiver<Result<(), RejectionKind>>,
    ) = oneshot::channel();
    let request: GameRequest = GameRequest::Respawn {
        member_id,
        loadout: Loadout::from(loadout_serial),
        team: TeamChoiceKind::from(team_serial_in),
        reply: reply_sender,
    };

    let respawn_result: Result<(), RejectionKind> = send_answered_request(&game_handle, request, reply_receiver).await;

    match respawn_result {
        Ok(()) => ConnectionFlow::Continue,
        Err(rejection) => reject(connection, RequestKind::Respawn, rejection).await,
    }
}

async fn update_appearance(connection: &mut Connection, appearance_serial: AppearanceSerial) -> ConnectionFlow {
    let gate: GameRequestGate = open_game_request(connection, RequestKind::UpdateAppearance).await;
    let (member_id, game_handle): (MemberId, GameHandle) = match gate {
        GameRequestGate::Open { member_id, game_handle } => (member_id, game_handle),
        GameRequestGate::Shut(flow) => return flow,
    };

    let request: GameRequest = GameRequest::UpdateAppearance {
        member_id,
        appearance: Appearance::from(appearance_serial),
    };
    let send_result: Result<(), RejectionKind> = send_request(&game_handle, request).await;

    match send_result {
        Ok(()) => ConnectionFlow::Continue,
        Err(rejection) => reject(connection, RequestKind::UpdateAppearance, rejection).await,
    }
}

/// Inputs outside a game, over the rate limit, or beyond a full input queue are dropped.
fn send_input(connection: &mut Connection, player_input_serial_in: PlayerInputSerialIn) -> ConnectionFlow {
    let input_conversion: Result<PlayerInput, AppError> = PlayerInput::try_from(player_input_serial_in);
    let Ok(input) = input_conversion else {
        return ConnectionFlow::Close(CloseCauseKind::ProtocolViolation(ProtocolViolationKind::Conversion));
    };

    let Some(joined_game) = &connection.joined_game else {
        return ConnectionFlow::Continue;
    };

    let input_token: TokenTakeKind = connection.input_bucket.take(Instant::now());

    match input_token {
        TokenTakeKind::Taken => {}
        TokenTakeKind::Refused => return ConnectionFlow::Continue,
        TokenTakeKind::Abused => {
            return ConnectionFlow::Close(CloseCauseKind::Reason(CloseReasonKind::RateLimitAbuse));
        }
    }

    let game_input: GameInput = GameInput {
        member_id: joined_game.member_id,
        input,
    };
    let send_result: Result<(), TrySendError<GameInput>> = joined_game.handle.inputs.try_send(game_input);

    if let Err(TrySendError::Full(_)) = send_result {
        connection.dropped_input_count += 1;
    }

    ConnectionFlow::Continue
}

/// Never rejected.
async fn request_snapshot(connection: &Connection) -> ConnectionFlow {
    let Some(joined_game) = &connection.joined_game else {
        return ConnectionFlow::Continue;
    };

    let request: GameRequest = GameRequest::RequestSnapshot {
        member_id: joined_game.member_id,
    };
    let _ = joined_game.handle.requests.send(request).await;

    ConnectionFlow::Continue
}

async fn leave_game(connection: &mut Connection) -> ConnectionFlow {
    let gate: GameRequestGate = open_game_request(connection, RequestKind::LeaveGame).await;
    let (member_id, game_handle): (MemberId, GameHandle) = match gate {
        GameRequestGate::Open { member_id, game_handle } => (member_id, game_handle),
        GameRequestGate::Shut(flow) => return flow,
    };

    let (reply_sender, reply_receiver): (oneshot::Sender<()>, oneshot::Receiver<()>) = oneshot::channel();
    let request: GameRequest = GameRequest::Leave {
        member_id,
        reply: reply_sender,
    };
    let send_result: Result<(), SendError<GameRequest>> = game_handle.requests.send(request).await;

    if send_result.is_ok() {
        let _ = reply_receiver.await;
    }

    connection.return_to_lobby();

    let left_game_frame: Bytes = connection_model::encode_frame(&MessageSerialOut::LeftGame);

    connection
        .send_to_writer(WriterCommand::DetachMembership {
            then_send: Some(left_game_frame),
        })
        .await
}

/// Lobby phase only, within the request rate.
async fn open_lobby_request(connection: &mut Connection, request_kind: RequestKind) -> RequestGate {
    if connection.phase() != ConnectionPhase::Lobby {
        return RequestGate::Shut(reject(connection, request_kind, RejectionKind::AlreadyInGame).await);
    }

    take_request_token(connection, request_kind).await
}

/// In a game only, within the request rate.
async fn open_game_request(connection: &mut Connection, request_kind: RequestKind) -> GameRequestGate {
    let Some(joined_game) = &connection.joined_game else {
        return GameRequestGate::Shut(reject(connection, request_kind, RejectionKind::NotInGame).await);
    };

    let member_id: MemberId = joined_game.member_id;
    let game_handle: GameHandle = joined_game.handle.clone();

    let token_gate: RequestGate = take_request_token(connection, request_kind).await;

    match token_gate {
        RequestGate::Open => GameRequestGate::Open { member_id, game_handle },
        RequestGate::Shut(flow) => GameRequestGate::Shut(flow),
    }
}

/// Sustained abuse closes the connection.
async fn take_request_token(connection: &mut Connection, request_kind: RequestKind) -> RequestGate {
    let request_token: TokenTakeKind = connection.request_bucket.take(Instant::now());

    match request_token {
        TokenTakeKind::Taken => RequestGate::Open,
        TokenTakeKind::Refused => RequestGate::Shut(reject(connection, request_kind, RejectionKind::RateLimited).await),
        TokenTakeKind::Abused => RequestGate::Shut(ConnectionFlow::Close(CloseCauseKind::Reason(
            CloseReasonKind::RateLimitAbuse,
        ))),
    }
}

async fn reject(connection: &Connection, request_kind: RequestKind, rejection: RejectionKind) -> ConnectionFlow {
    let rejection_message: MessageSerialOut = MessageSerialOut::RequestRejected {
        request: RequestKindSerialOut::from(&request_kind),
        reason: RejectionKindSerialOut::from(&rejection),
    };

    connection.send_message(&rejection_message).await
}
