use bitcode::{Decode, Encode};

use crate::error::AppError;
use crate::game::GameSettings;
use crate::protocol::GameSettingsSerialOut;
use crate::replay::ReplayHeader;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ReplayHeaderSerialOut {
    pub settings: GameSettingsSerialOut,
    pub seed: u64,
}

impl From<&ReplayHeader> for ReplayHeaderSerialOut {
    fn from(header: &ReplayHeader) -> ReplayHeaderSerialOut {
        ReplayHeaderSerialOut {
            settings: GameSettingsSerialOut::from(&header.settings),
            seed: header.seed,
        }
    }
}

impl TryFrom<ReplayHeaderSerialOut> for ReplayHeader {
    type Error = AppError;

    fn try_from(header_serial_out: ReplayHeaderSerialOut) -> Result<ReplayHeader, AppError> {
        Ok(ReplayHeader {
            settings: GameSettings::try_from(header_serial_out.settings)?,
            seed: header_serial_out.seed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, test_fixture};
    use crate::world::WorldShapeKind;

    fn create_header() -> ReplayHeader {
        ReplayHeader {
            settings: test_fixture::create_settings(GameModeKind::Skirmish, WorldShapeKind::Ellipse, 900),
            seed: 0x0123_4567_89ab_cdef,
        }
    }

    #[test]
    fn try_from_restores_the_header() {
        let header: ReplayHeader = create_header();

        assert_eq!(
            ReplayHeader::try_from(ReplayHeaderSerialOut::from(&header)).unwrap(),
            header
        );
    }

    #[test]
    fn try_from_rejects_invalid_settings() {
        let mut header_serial_out: ReplayHeaderSerialOut = ReplayHeaderSerialOut::from(&create_header());
        header_serial_out.settings.team_count = None;

        assert!(ReplayHeader::try_from(header_serial_out).is_err());
    }
}
