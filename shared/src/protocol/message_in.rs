use bitcode::{Decode, Encode};

use crate::ability::{AbilityPressSet, AimVector, Loadout};
use crate::error::AppError;
use crate::game::{GameModeKind, GameSettings, PlayerInput, Tick};
use crate::geometry::SubpixelPoint;
use crate::member::{Joiner, TeamChoiceKind, TeamKind};
use crate::protocol::{
    AimVectorSerial, AppearanceSerial, GameModeKindSerial, LoadoutSerial, RejectionKind, SubpixelPointSerial,
    TeamKindSerial, WorldShapeKindSerial,
};
use crate::protocol::{input_bundle_serial, protocol_limits};
use crate::world::WorldShapeKind;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum MessageSerialIn {
    SubscribeGameList,
    UnsubscribeGameList,
    CreateGame {
        settings: GameSettingsSerialIn,
        password: Option<String>,
        joiner: JoinerSerialIn,
    },
    JoinGame {
        game_id: u32,
        password: Option<String>,
        joiner: JoinerSerialIn,
    },
    SpectateGame {
        game_id: u32,
        password: Option<String>,
        screen_name: String,
    },
    Respawn {
        loadout: LoadoutSerial,
        team: TeamChoiceKindSerialIn,
    },
    UpdateAppearance {
        appearance: AppearanceSerial,
    },
    Input(PlayerInputSerialIn),
    RequestSnapshot {
        client_tick: u32,
    },
    LeaveGame,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PlayerInputSerialIn {
    pub client_tick: u32,
    pub cursor: SubpixelPointSerial,
    pub ability_presses: u8,
    pub aim: Option<AimVectorSerial>,
}

impl TryFrom<PlayerInputSerialIn> for PlayerInput {
    type Error = AppError;

    fn try_from(player_input_serial_in: PlayerInputSerialIn) -> Result<PlayerInput, AppError> {
        let ability_presses: AbilityPressSet =
            input_bundle_serial::convert_ability_presses(player_input_serial_in.ability_presses)?;
        let has_shot_slot_press: bool =
            ability_presses.contains(AbilityPressSet::FIRST) || ability_presses.contains(AbilityPressSet::SECOND);

        if player_input_serial_in.aim.is_some() && !has_shot_slot_press {
            return Err(AppError::new("an aim comes only with a first or second ability press"));
        }

        Ok(PlayerInput {
            client_tick: Tick(player_input_serial_in.client_tick),
            cursor: SubpixelPoint::try_from(player_input_serial_in.cursor)?,
            ability_presses,
            aim: player_input_serial_in.aim.map(AimVector::from),
        })
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

    fn create_player_input_serial_in(ability_presses: u8, aim: Option<AimVectorSerial>) -> PlayerInputSerialIn {
        PlayerInputSerialIn {
            client_tick: 41,
            cursor: SubpixelPointSerial { x: 409_600, y: -1 },
            ability_presses,
            aim,
        }
    }

    #[test]
    fn player_input_try_from_accepts_an_aim_with_a_shot_slot_press() {
        let aim_serial: AimVectorSerial = AimVectorSerial { x: 30, y: -4 };
        let player_input: PlayerInput =
            PlayerInput::try_from(create_player_input_serial_in(0b0010, Some(aim_serial))).unwrap();

        assert_eq!(
            player_input,
            PlayerInput {
                client_tick: Tick(41),
                cursor: SubpixelPoint { x: 409_600, y: -1 },
                ability_presses: AbilityPressSet::SECOND,
                aim: Some(AimVector { x: 30, y: -4 }),
            },
        );
        assert!(PlayerInput::try_from(create_player_input_serial_in(0b0001, Some(aim_serial))).is_ok());
        assert!(PlayerInput::try_from(create_player_input_serial_in(0b0001, None)).is_ok());
    }

    #[test]
    fn player_input_try_from_rejects_an_aim_without_a_shot_slot_press() {
        let aim_serial: AimVectorSerial = AimVectorSerial { x: 30, y: -4 };

        assert!(PlayerInput::try_from(create_player_input_serial_in(0b1100, Some(aim_serial))).is_err());
    }

    #[test]
    fn player_input_try_from_rejects_unknown_press_bits() {
        assert!(PlayerInput::try_from(create_player_input_serial_in(0b1000_0000, None)).is_err());
    }

    #[test]
    fn player_input_try_from_rejects_a_cursor_beyond_the_coordinate_limit() {
        let mut player_input_serial_in: PlayerInputSerialIn = create_player_input_serial_in(0, None);
        player_input_serial_in.cursor.y = 268_435_457;

        assert!(PlayerInput::try_from(player_input_serial_in).is_err());
    }
}
