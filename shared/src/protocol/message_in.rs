use bitcode::{Decode, Encode};

use crate::ability::Loadout;
use crate::game::{GameModeKind, GameSettings};
use crate::member::{Joiner, TeamChoiceKind, TeamKind};
use crate::protocol::protocol_limits;
use crate::protocol::{GameModeKindSerial, LoadoutSerial, RejectionKind, TeamKindSerial, WorldShapeKindSerial};
use crate::world::WorldShapeKind;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct GameSettingsSerialIn {
    pub title: String,
    pub mode: GameModeKindSerial,
    pub world_shape: WorldShapeKindSerial,
    pub world_size_pixels: u32,
    pub player_minimum: Option<u8>,
    pub player_cap: u8,
    pub team_count: Option<u8>,
    pub leaderboard_length: u8,
}

impl TryFrom<GameSettingsSerialIn> for GameSettings {
    type Error = RejectionKind;

    fn try_from(settings_serial_in: GameSettingsSerialIn) -> Result<GameSettings, RejectionKind> {
        let settings: GameSettings = GameSettings {
            title: protocol_limits::normalize_title(&settings_serial_in.title)?,
            mode: GameModeKind::from(settings_serial_in.mode),
            world_shape: WorldShapeKind::from(settings_serial_in.world_shape),
            // Created games are square.
            world_width_pixels: settings_serial_in.world_size_pixels,
            world_height_pixels: settings_serial_in.world_size_pixels,
            player_minimum: settings_serial_in.player_minimum,
            player_cap: settings_serial_in.player_cap,
            team_count: settings_serial_in.team_count,
            leaderboard_length: settings_serial_in.leaderboard_length,
        };

        settings.validate()?;

        Ok(settings)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct JoinerSerialIn {
    pub screen_name: String,
    pub loadout: LoadoutSerial,
    pub team: TeamChoiceKindSerialIn,
}

impl TryFrom<JoinerSerialIn> for Joiner {
    type Error = RejectionKind;

    fn try_from(joiner_serial_in: JoinerSerialIn) -> Result<Joiner, RejectionKind> {
        Ok(Joiner {
            screen_name: protocol_limits::normalize_screen_name(&joiner_serial_in.screen_name)?,
            loadout: Loadout::from(joiner_serial_in.loadout),
            team: TeamChoiceKind::from(joiner_serial_in.team),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum TeamChoiceKindSerialIn {
    Auto,
    Team(TeamKindSerial),
}

impl From<TeamChoiceKindSerialIn> for TeamChoiceKind {
    fn from(team_choice_serial_in: TeamChoiceKindSerialIn) -> TeamChoiceKind {
        match team_choice_serial_in {
            TeamChoiceKindSerialIn::Auto => TeamChoiceKind::Auto,
            TeamChoiceKindSerialIn::Team(team_serial) => TeamChoiceKind::Team(TeamKind::from(team_serial)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::protocol::{RangeBoundKind, SettingFieldKind};

    fn create_settings_serial_in() -> GameSettingsSerialIn {
        GameSettingsSerialIn {
            title: String::from("  Arena  "),
            mode: GameModeKindSerial::Survival,
            world_shape: WorldShapeKindSerial::Ellipse,
            world_size_pixels: 800,
            player_minimum: Some(4),
            player_cap: 16,
            team_count: None,
            leaderboard_length: 10,
        }
    }

    fn create_joiner_serial_in() -> JoinerSerialIn {
        JoinerSerialIn {
            screen_name: String::from(" Blob "),
            loadout: LoadoutSerial::from(&test_fixture::create_loadout()),
            team: TeamChoiceKindSerialIn::Team(TeamKindSerial::Pink),
        }
    }

    #[test]
    fn game_settings_try_from_trims_the_title_and_makes_the_world_square() {
        let settings: GameSettings = GameSettings::try_from(create_settings_serial_in()).unwrap();

        assert_eq!(
            settings,
            GameSettings {
                title: String::from("Arena"),
                mode: GameModeKind::Survival,
                world_shape: WorldShapeKind::Ellipse,
                world_width_pixels: 800,
                world_height_pixels: 800,
                player_minimum: Some(4),
                player_cap: 16,
                team_count: None,
                leaderboard_length: 10,
            },
        );
    }

    #[test]
    fn game_settings_try_from_rejects_a_blank_title() {
        let mut settings_serial_in: GameSettingsSerialIn = create_settings_serial_in();
        settings_serial_in.title = String::from("   ");

        assert_eq!(
            GameSettings::try_from(settings_serial_in),
            Err(RejectionKind::TitleEmpty)
        );
    }

    #[test]
    fn game_settings_try_from_applies_the_settings_rules() {
        let mut small_world_serial_in: GameSettingsSerialIn = create_settings_serial_in();
        small_world_serial_in.world_size_pixels = 299;
        let mut low_cap_serial_in: GameSettingsSerialIn = create_settings_serial_in();
        low_cap_serial_in.player_cap = 3;

        assert_eq!(
            GameSettings::try_from(small_world_serial_in),
            Err(RejectionKind::SettingOutOfRange {
                field: SettingFieldKind::WorldSize,
                bound: RangeBoundKind::Below,
            }),
        );
        assert_eq!(
            GameSettings::try_from(low_cap_serial_in),
            Err(RejectionKind::PlayerCapBelowMinimum)
        );
    }

    #[test]
    fn joiner_try_from_trims_the_screen_name_and_converts_the_team_choice() {
        let joiner: Joiner = Joiner::try_from(create_joiner_serial_in()).unwrap();

        assert_eq!(
            joiner,
            Joiner {
                screen_name: String::from("Blob"),
                loadout: test_fixture::create_loadout(),
                team: TeamChoiceKind::Team(TeamKind::Pink),
            },
        );
    }

    #[test]
    fn joiner_try_from_rejects_an_invalid_screen_name() {
        let mut joiner_serial_in: JoinerSerialIn = create_joiner_serial_in();
        joiner_serial_in.screen_name = "n".repeat(65);

        assert_eq!(
            Joiner::try_from(joiner_serial_in),
            Err(RejectionKind::ScreenNameTooLong)
        );
    }

    #[test]
    fn team_choice_kind_from_converts_auto() {
        assert_eq!(TeamChoiceKind::from(TeamChoiceKindSerialIn::Auto), TeamChoiceKind::Auto);
    }
}
