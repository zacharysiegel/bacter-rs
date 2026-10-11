use std::time::Duration;

use actix_ws::{CloseCode, CloseReason, Closed, Session};
use bytes::Bytes;
use tokio::sync::mpsc;
use tokio::time::error::Elapsed;

use crate::socket::CloseCauseKind;

pub const SOCKET_WRITE_TIMEOUT: Duration = Duration::from_secs(5);
pub const WRITER_COMMAND_CHANNEL_CAPACITY: usize = 32;

pub enum WriterCommand {
    Frame(Bytes),
    AttachMembership(mpsc::Receiver<Bytes>),
    /// Drops the membership queue unread, so no frame of the old game follows `then_send`.
    DetachMembership {
        then_send: Option<Bytes>,
    },
    Ping,
    Pong(Bytes),
    Close(CloseCauseKind),
}

enum WriterEventKind {
    Command(Option<WriterCommand>),
    MembershipFrame(Option<Bytes>),
    LobbyFrame(Option<Bytes>),
}

enum WriterFlow {
    Continue,
    Close(CloseCauseKind),
    End,
}

/// The only owner of the session; a write which does not finish within the timeout drops it without a close frame.
pub async fn run_socket_writer(
    mut session: Session,
    mut commands: mpsc::Receiver<WriterCommand>,
    mut lobby_frames: mpsc::Receiver<Bytes>,
) {
    let mut membership_frames: Option<mpsc::Receiver<Bytes>> = None;
    let mut is_lobby_open: bool = true;

    loop {
        let event: WriterEventKind = tokio::select! {
            biased;
            command = commands.recv() => WriterEventKind::Command(command),
            frame = receive_membership_frame(&mut membership_frames) => WriterEventKind::MembershipFrame(frame),
            frame = lobby_frames.recv(), if is_lobby_open => WriterEventKind::LobbyFrame(frame),
        };

        let flow: WriterFlow = match event {
            WriterEventKind::Command(Some(command)) => {
                handle_command(&mut session, &mut membership_frames, command).await
            }
            WriterEventKind::Command(None) => WriterFlow::End,
            WriterEventKind::MembershipFrame(Some(frame)) | WriterEventKind::LobbyFrame(Some(frame)) => {
                write_frame(&mut session, frame).await
            }
            WriterEventKind::MembershipFrame(None) => {
                membership_frames = None;

                WriterFlow::Continue
            }
            WriterEventKind::LobbyFrame(None) => {
                is_lobby_open = false;

                WriterFlow::Continue
            }
        };

        match flow {
            WriterFlow::Continue => {}
            WriterFlow::Close(cause) => {
                close_session(session, cause).await;

                break;
            }
            WriterFlow::End => break,
        }
    }
}

pub async fn close_session(session: Session, cause: CloseCauseKind) {
    let close_reason: CloseReason = CloseReason {
        code: CloseCode::from(cause.close_code()),
        description: cause.description(),
    };

    let _ = tokio::time::timeout(SOCKET_WRITE_TIMEOUT, session.close(Some(close_reason))).await;
}

/// Never finishes without a membership.
async fn receive_membership_frame(membership_frames: &mut Option<mpsc::Receiver<Bytes>>) -> Option<Bytes> {
    match membership_frames {
        Some(membership_frames) => membership_frames.recv().await,
        None => std::future::pending().await,
    }
}

async fn handle_command(
    session: &mut Session,
    membership_frames: &mut Option<mpsc::Receiver<Bytes>>,
    command: WriterCommand,
) -> WriterFlow {
    match command {
        WriterCommand::Frame(frame) => write_frame(session, frame).await,
        WriterCommand::AttachMembership(attached_frames) => {
            *membership_frames = Some(attached_frames);

            WriterFlow::Continue
        }
        WriterCommand::DetachMembership { then_send } => {
            *membership_frames = None;

            match then_send {
                Some(frame) => write_frame(session, frame).await,
                None => WriterFlow::Continue,
            }
        }
        WriterCommand::Ping => {
            let ping_result: Result<Result<(), Closed>, Elapsed> =
                tokio::time::timeout(SOCKET_WRITE_TIMEOUT, session.ping(b"")).await;

            get_write_flow(ping_result)
        }
        WriterCommand::Pong(payload) => {
            let pong_result: Result<Result<(), Closed>, Elapsed> =
                tokio::time::timeout(SOCKET_WRITE_TIMEOUT, session.pong(&payload)).await;

            get_write_flow(pong_result)
        }
        WriterCommand::Close(cause) => WriterFlow::Close(cause),
    }
}

async fn write_frame(session: &mut Session, frame: Bytes) -> WriterFlow {
    let write_result: Result<Result<(), Closed>, Elapsed> =
        tokio::time::timeout(SOCKET_WRITE_TIMEOUT, session.binary(frame)).await;

    get_write_flow(write_result)
}

fn get_write_flow(write_result: Result<Result<(), Closed>, Elapsed>) -> WriterFlow {
    match write_result {
        Ok(Ok(())) => WriterFlow::Continue,
        Ok(Err(Closed)) => WriterFlow::End,
        Err(_) => {
            log::info!("socket write timed out");

            WriterFlow::End
        }
    }
}
