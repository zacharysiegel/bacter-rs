use shared::error::AppError;
use shared::protocol;
use shared::protocol::{MessageSerialIn, MessageSerialOut};

const CLIENT_TO_SERVER_NAME: &str = "client-to-server";
const SERVER_TO_CLIENT_NAME: &str = "server-to-client";
pub const DIRECTION_NAMES: [&str; 2] = [CLIENT_TO_SERVER_NAME, SERVER_TO_CLIENT_NAME];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectionKind {
    ClientToServer,
    ServerToClient,
}

impl TryFrom<&str> for DirectionKind {
    type Error = AppError;

    fn try_from(name: &str) -> Result<DirectionKind, AppError> {
        match name {
            CLIENT_TO_SERVER_NAME => Ok(DirectionKind::ClientToServer),
            SERVER_TO_CLIENT_NAME => Ok(DirectionKind::ServerToClient),
            _ => Err(AppError::new(&format!("unknown direction {name:?}"))),
        }
    }
}

/// The `Debug` form of the decoded message.
pub fn dump_frame(frame_bytes: &[u8], direction: DirectionKind) -> Result<String, AppError> {
    let dump: String = match direction {
        DirectionKind::ClientToServer => {
            let message_serial_in: MessageSerialIn = protocol::decode_message_in(frame_bytes)?;

            format!("{message_serial_in:#?}\n")
        }
        DirectionKind::ServerToClient => {
            let message_serial_out: MessageSerialOut = protocol::decode_message_out(frame_bytes)?;

            format!("{message_serial_out:#?}\n")
        }
    };

    Ok(dump)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_from_reads_every_direction_name() {
        let directions: Vec<DirectionKind> =
            DIRECTION_NAMES.iter().map(|name| DirectionKind::try_from(*name).unwrap()).collect();

        assert_eq!(
            directions,
            vec![DirectionKind::ClientToServer, DirectionKind::ServerToClient]
        );
        assert!(DirectionKind::try_from("sideways").is_err());
    }

    #[test]
    fn dump_frame_prints_a_server_message() {
        let frame_bytes: Vec<u8> = protocol::encode_message_out(&MessageSerialOut::StateChecksum {
            tick: 14,
            checksum: 255,
        });

        assert_eq!(
            dump_frame(&frame_bytes, DirectionKind::ServerToClient).unwrap(),
            "StateChecksum {\n    tick: 14,\n    checksum: 255,\n}\n",
        );
    }

    #[test]
    fn dump_frame_prints_a_client_message() {
        let frame_bytes: Vec<u8> = protocol::encode_message_in(&MessageSerialIn::RequestSnapshot { client_tick: 3 });

        assert_eq!(
            dump_frame(&frame_bytes, DirectionKind::ClientToServer).unwrap(),
            "RequestSnapshot {\n    client_tick: 3,\n}\n",
        );
    }

    #[test]
    fn dump_frame_rejects_bytes_of_the_other_direction() {
        let frame_bytes: Vec<u8> = protocol::encode_message_out(&MessageSerialOut::JoinAccepted {
            game_id: 1,
            member_id: 2,
        });

        assert!(dump_frame(&frame_bytes, DirectionKind::ClientToServer).is_err());
    }
}
