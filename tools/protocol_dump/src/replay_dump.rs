use shared::error::AppError;
use shared::replay;
use shared::replay::{ReplayLog, ReplayReadError, TickChecksum};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayOutputKind {
    /// The header and every bundle, in their `Debug` form.
    Bundles,
    /// The checksum after each tick of a re-run.
    Checksums,
}

pub fn dump_replay(replay_bytes: &[u8], output: ReplayOutputKind) -> Result<String, AppError> {
    let replay_log: ReplayLog = replay::read_replay(replay_bytes).map_err(get_replay_read_error)?;

    match output {
        ReplayOutputKind::Bundles => Ok(format_replay_log(&replay_log)),
        ReplayOutputKind::Checksums => {
            let tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log)
                .map_err(|step_error| AppError::new(&format!("the replay does not step: {step_error:?}")))?;

            Ok(replay::format_tick_checksums(&tick_checksums))
        }
    }
}

fn format_replay_log(replay_log: &ReplayLog) -> String {
    let mut dump: String = format!("{:#?}\n", replay_log.header);

    for bundle in &replay_log.input_bundles {
        dump.push_str(&format!("{bundle:#?}\n"));
    }

    dump
}

fn get_replay_read_error(replay_read_error: ReplayReadError) -> AppError {
    match replay_read_error {
        ReplayReadError::ProtocolVersionMismatch {
            file_version,
            current_version,
        } => AppError::new(&format!(
            "the replay has protocol version {file_version}, not {current_version}"
        )),
        ReplayReadError::Truncated => AppError::new("the replay is truncated"),
        ReplayReadError::MissingHeader => AppError::new("the replay has no header"),
        ReplayReadError::InvalidRecord { record_index, error } => {
            AppError::from_error(&format!("replay record {record_index} is invalid"), Box::new(error))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::RangeInclusive;

    use shared::game::{GameModeKind, GameSettings, InputBundle, StepError, Tick};
    use shared::protocol;
    use shared::protocol::ReplayHeaderSerialOut;
    use shared::replay::ReplayHeader;
    use shared::world::WorldShapeKind;

    const CORRUPTED_RECORD: [u8; 3] = [0xff, 0xff, 0xff];

    fn create_replay_log() -> ReplayLog {
        create_replay_log_with_bundle_ticks(1..=3)
    }

    fn create_replay_log_with_bundle_ticks(bundle_ticks: RangeInclusive<u32>) -> ReplayLog {
        ReplayLog {
            header: ReplayHeader {
                settings: GameSettings {
                    title: String::from("Dumped game"),
                    mode: GameModeKind::FreeForAll,
                    world_shape: WorldShapeKind::Rectangle,
                    world_width_pixels: 800,
                    world_height_pixels: 800,
                    player_minimum: None,
                    player_cap: 8,
                    team_count: None,
                    leaderboard_length: 10,
                },
                seed: 5,
            },
            input_bundles: bundle_ticks
                .map(|tick| InputBundle {
                    tick: Tick(tick),
                    member_events: Vec::new(),
                    player_inputs: Vec::new(),
                })
                .collect(),
        }
    }

    fn create_replay_bytes() -> Vec<u8> {
        replay::write_replay(&create_replay_log())
    }

    fn get_replay_bytes_with_a_corrupted_bundle_record() -> Vec<u8> {
        let corrupted_record_length: u32 = u32::try_from(CORRUPTED_RECORD.len()).unwrap();
        let mut replay_bytes: Vec<u8> = replay::get_version_prefix().to_vec();
        replay_bytes.extend(replay::get_header_record_bytes(&ReplayHeaderSerialOut::from(
            &create_replay_log().header,
        )));
        replay_bytes.extend(corrupted_record_length.to_le_bytes());
        replay_bytes.extend(CORRUPTED_RECORD);

        replay_bytes
    }

    #[test]
    fn dump_replay_prints_the_checksum_lines_of_a_rerun() {
        let replay_bytes: Vec<u8> = create_replay_bytes();
        let replay_log: ReplayLog = replay::read_replay(&replay_bytes).unwrap();
        let expected_dump: String = replay::format_tick_checksums(&replay::run_replay(&replay_log).unwrap());

        assert_eq!(
            dump_replay(&replay_bytes, ReplayOutputKind::Checksums).unwrap(),
            expected_dump
        );
    }

    #[test]
    fn dump_replay_reports_a_replay_which_does_not_step() {
        let replay_bytes: Vec<u8> = replay::write_replay(&create_replay_log_with_bundle_ticks(2..=4));

        let error: AppError = dump_replay(&replay_bytes, ReplayOutputKind::Checksums).unwrap_err();

        assert_eq!(
            error.message,
            format!(
                "Error: the replay does not step: {:?}",
                StepError::TickMismatch {
                    expected: Tick(1),
                    received: Tick(2),
                },
            ),
        );
    }

    #[test]
    fn dump_replay_prints_the_header_then_every_bundle() {
        let dump: String = dump_replay(&create_replay_bytes(), ReplayOutputKind::Bundles).unwrap();

        assert!(dump.starts_with("ReplayHeader {\n"));
        assert_eq!(dump.matches("InputBundle {\n").count(), 3);
    }

    #[test]
    fn dump_replay_names_another_protocol_version() {
        let other_protocol_version: u16 = protocol::PROTOCOL_VERSION + 1;
        let version_prefix_byte_count: usize = replay::get_version_prefix().len();
        let mut replay_bytes: Vec<u8> = create_replay_bytes();
        replay_bytes[..version_prefix_byte_count].copy_from_slice(&other_protocol_version.to_le_bytes());

        let error: AppError = dump_replay(&replay_bytes, ReplayOutputKind::Bundles).unwrap_err();

        assert_eq!(
            error.message,
            format!(
                "Error: the replay has protocol version {other_protocol_version}, not {}",
                protocol::PROTOCOL_VERSION,
            ),
        );
    }

    #[test]
    fn dump_replay_reports_a_truncated_replay() {
        let error: AppError = dump_replay(&[1], ReplayOutputKind::Bundles).unwrap_err();

        assert_eq!(error.message, "Error: the replay is truncated");
    }

    #[test]
    fn dump_replay_reports_a_replay_without_a_header() {
        let error: AppError = dump_replay(&replay::get_version_prefix(), ReplayOutputKind::Bundles).unwrap_err();

        assert_eq!(error.message, "Error: the replay has no header");
    }

    #[test]
    fn dump_replay_names_the_invalid_record_and_keeps_its_error() {
        let replay_bytes: Vec<u8> = get_replay_bytes_with_a_corrupted_bundle_record();
        let expected_sub_error: AppError = protocol::decode_replay_bundle(&CORRUPTED_RECORD).unwrap_err();

        let error: AppError = dump_replay(&replay_bytes, ReplayOutputKind::Bundles).unwrap_err();
        let sub_error: &AppError =
            error.sub_error.as_ref().and_then(|sub_error| sub_error.downcast_ref::<AppError>()).unwrap();

        assert_eq!(error.message, "Error: replay record 1 is invalid");
        assert_eq!(sub_error.message, expected_sub_error.message);
    }
}
