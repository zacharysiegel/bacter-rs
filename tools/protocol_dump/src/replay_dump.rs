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
    use shared::game::{GameModeKind, GameSettings, InputBundle, Tick};
    use shared::replay::ReplayHeader;
    use shared::world::WorldShapeKind;

    fn create_replay_bytes() -> Vec<u8> {
        let replay_log: ReplayLog = ReplayLog {
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
            input_bundles: (1..=3)
                .map(|tick| InputBundle {
                    tick: Tick(tick),
                    member_events: Vec::new(),
                    player_inputs: Vec::new(),
                })
                .collect(),
        };

        replay::write_replay(&replay_log)
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
    fn dump_replay_prints_the_header_then_every_bundle() {
        let dump: String = dump_replay(&create_replay_bytes(), ReplayOutputKind::Bundles).unwrap();

        assert!(dump.starts_with("ReplayHeader {\n"));
        assert_eq!(dump.matches("InputBundle {\n").count(), 3);
    }

    #[test]
    fn dump_replay_names_another_protocol_version() {
        let mut replay_bytes: Vec<u8> = create_replay_bytes();
        replay_bytes[..2].copy_from_slice(&9_u16.to_le_bytes());

        let error: AppError = dump_replay(&replay_bytes, ReplayOutputKind::Bundles).unwrap_err();

        assert_eq!(error.message, "Error: the replay has protocol version 9, not 1");
    }
}
