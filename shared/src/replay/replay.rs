use crate::error::AppError;
use crate::game::{GameSettings, InputBundle};
use crate::protocol;
use crate::protocol::{InputBundleSerialOut, ReplayHeaderSerialOut};

const VERSION_PREFIX_BYTE_COUNT: usize = 2;
const RECORD_LENGTH_BYTE_COUNT: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayHeader {
    pub settings: GameSettings,
    pub seed: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayLog {
    pub header: ReplayHeader,
    /// One per tick from tick 1.
    pub input_bundles: Vec<InputBundle>,
}

#[derive(Debug)]
pub enum ReplayReadError {
    ProtocolVersionMismatch {
        file_version: u16,
        current_version: u16,
    },
    Truncated,
    MissingHeader,
    /// Record 0 is the header.
    InvalidRecord {
        record_index: u32,
        error: AppError,
    },
}

/// Little-endian, ahead of the encoded records.
pub fn get_version_prefix() -> [u8; VERSION_PREFIX_BYTE_COUNT] {
    protocol::PROTOCOL_VERSION.to_le_bytes()
}

pub fn get_header_record_bytes(header_serial_out: &ReplayHeaderSerialOut) -> Vec<u8> {
    get_length_prefixed_record(&protocol::encode_replay_header(header_serial_out))
}

pub fn get_bundle_record_bytes(bundle_serial_out: &InputBundleSerialOut) -> Vec<u8> {
    get_length_prefixed_record(&protocol::encode_replay_bundle(bundle_serial_out))
}

pub fn write_replay(replay_log: &ReplayLog) -> Vec<u8> {
    let mut replay_bytes: Vec<u8> = get_version_prefix().to_vec();
    replay_bytes.extend(get_header_record_bytes(&ReplayHeaderSerialOut::from(
        &replay_log.header,
    )));

    for bundle in &replay_log.input_bundles {
        replay_bytes.extend(get_bundle_record_bytes(&InputBundleSerialOut::from(bundle)));
    }

    replay_bytes
}

pub fn read_replay(replay_bytes: &[u8]) -> Result<ReplayLog, ReplayReadError> {
    let Some((version_bytes, record_stream_bytes)): Option<(&[u8; VERSION_PREFIX_BYTE_COUNT], &[u8])> =
        replay_bytes.split_first_chunk::<VERSION_PREFIX_BYTE_COUNT>()
    else {
        return Err(ReplayReadError::Truncated);
    };

    let file_version: u16 = u16::from_le_bytes(*version_bytes);

    if file_version != protocol::PROTOCOL_VERSION {
        return Err(ReplayReadError::ProtocolVersionMismatch {
            file_version,
            current_version: protocol::PROTOCOL_VERSION,
        });
    }

    let records: Vec<&[u8]> = split_records(record_stream_bytes)?;
    let Some((header_record, bundle_records)): Option<(&&[u8], &[&[u8]])> = records.split_first() else {
        return Err(ReplayReadError::MissingHeader);
    };

    let header: ReplayHeader = read_header(header_record)?;
    let mut input_bundles: Vec<InputBundle> = Vec::new();

    for (bundle_record, record_index) in bundle_records.iter().zip(1_u32..) {
        let bundle: InputBundle =
            read_bundle(bundle_record).map_err(|error| ReplayReadError::InvalidRecord { record_index, error })?;
        input_bundles.push(bundle);
    }

    Ok(ReplayLog { header, input_bundles })
}

fn get_length_prefixed_record(record: &[u8]) -> Vec<u8> {
    let record_length: u32 = u32::try_from(record.len()).expect("a replay record is shorter than 4 GiB");
    let mut record_bytes: Vec<u8> = record_length.to_le_bytes().to_vec();
    record_bytes.extend_from_slice(record);

    record_bytes
}

fn split_records(record_stream_bytes: &[u8]) -> Result<Vec<&[u8]>, ReplayReadError> {
    let mut records: Vec<&[u8]> = Vec::new();
    let mut remaining_bytes: &[u8] = record_stream_bytes;

    while !remaining_bytes.is_empty() {
        let Some((length_bytes, after_length_bytes)): Option<(&[u8; RECORD_LENGTH_BYTE_COUNT], &[u8])> =
            remaining_bytes.split_first_chunk::<RECORD_LENGTH_BYTE_COUNT>()
        else {
            return Err(ReplayReadError::Truncated);
        };

        let record_length: usize =
            usize::try_from(u32::from_le_bytes(*length_bytes)).map_err(|_| ReplayReadError::Truncated)?;

        if after_length_bytes.len() < record_length {
            return Err(ReplayReadError::Truncated);
        }

        let (record, after_record_bytes): (&[u8], &[u8]) = after_length_bytes.split_at(record_length);
        records.push(record);
        remaining_bytes = after_record_bytes;
    }

    Ok(records)
}

fn read_header(header_record: &[u8]) -> Result<ReplayHeader, ReplayReadError> {
    protocol::decode_replay_header(header_record)
        .and_then(ReplayHeader::try_from)
        .map_err(|error| ReplayReadError::InvalidRecord { record_index: 0, error })
}

fn read_bundle(bundle_record: &[u8]) -> Result<InputBundle, AppError> {
    InputBundle::try_from(protocol::decode_replay_bundle(bundle_record)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, MemberEvent, Tick, test_fixture};
    use crate::member::{MemberId, MemberRoleKind};
    use crate::world::WorldShapeKind;

    fn create_replay_log() -> ReplayLog {
        ReplayLog {
            header: ReplayHeader {
                settings: test_fixture::create_settings(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800),
                seed: 17,
            },
            input_bundles: vec![
                InputBundle {
                    tick: Tick(1),
                    member_events: vec![MemberEvent::Joined {
                        member_id: MemberId(0),
                        screen_name: String::from("Blob"),
                        role: MemberRoleKind::Spectator,
                        loadout: None,
                        team: None,
                    }],
                    player_inputs: Vec::new(),
                },
                InputBundle {
                    tick: Tick(2),
                    member_events: Vec::new(),
                    player_inputs: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn read_replay_restores_the_written_log() {
        let replay_log: ReplayLog = create_replay_log();

        assert_eq!(read_replay(&write_replay(&replay_log)).unwrap(), replay_log);
    }

    #[test]
    fn write_replay_starts_with_the_little_endian_protocol_version() {
        let replay_bytes: Vec<u8> = write_replay(&create_replay_log());

        assert_eq!(replay_bytes[..2], protocol::PROTOCOL_VERSION.to_le_bytes());
    }

    #[test]
    fn read_replay_rejects_another_protocol_version_before_decoding() {
        let mut replay_bytes: Vec<u8> = write_replay(&create_replay_log());
        replay_bytes[..2].copy_from_slice(&7_u16.to_le_bytes());
        replay_bytes.truncate(5);

        let replay_read_error: ReplayReadError = read_replay(&replay_bytes).unwrap_err();

        assert!(matches!(
            replay_read_error,
            ReplayReadError::ProtocolVersionMismatch {
                file_version: 7,
                current_version: protocol::PROTOCOL_VERSION,
            },
        ));
    }

    #[test]
    fn read_replay_rejects_a_truncated_record() {
        let replay_bytes: Vec<u8> = write_replay(&create_replay_log());

        let replay_read_error: ReplayReadError = read_replay(&replay_bytes[..replay_bytes.len() - 1]).unwrap_err();

        assert!(matches!(replay_read_error, ReplayReadError::Truncated));
        assert!(matches!(read_replay(&[1]).unwrap_err(), ReplayReadError::Truncated));
    }

    #[test]
    fn read_replay_rejects_a_file_without_a_header() {
        let replay_read_error: ReplayReadError = read_replay(&get_version_prefix()).unwrap_err();

        assert!(matches!(replay_read_error, ReplayReadError::MissingHeader));
    }

    #[test]
    fn read_replay_names_the_invalid_record() {
        let replay_log: ReplayLog = create_replay_log();
        let mut replay_bytes: Vec<u8> = get_version_prefix().to_vec();
        replay_bytes.extend(get_header_record_bytes(&ReplayHeaderSerialOut::from(
            &replay_log.header,
        )));
        replay_bytes.extend(get_length_prefixed_record(&[0xff, 0xff, 0xff]));

        let replay_read_error: ReplayReadError = read_replay(&replay_bytes).unwrap_err();

        assert!(matches!(
            replay_read_error,
            ReplayReadError::InvalidRecord { record_index: 1, .. }
        ));
    }
}
