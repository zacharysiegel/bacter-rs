use crate::protocol::protocol_limits;
use crate::protocol::{RangeBoundKind, RejectionKind, SettingFieldKind};
use crate::world::WorldShapeKind;

const FREE_FOR_ALL_CODE: &str = "ffa";
const SKIRMISH_CODE: &str = "skm";
const SURVIVAL_CODE: &str = "srv";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSettings {
    pub title: String,
    pub mode: GameModeKind,
    pub world_shape: WorldShapeKind,
    pub world_width_pixels: u32,
    pub world_height_pixels: u32,
    /// Some only in survival.
    pub player_minimum: Option<u8>,
    /// Alive organisms, not members.
    pub player_cap: u8,
    /// Some only in skirmish.
    pub team_count: Option<u8>,
    pub leaderboard_length: u8,
}

impl GameSettings {
    /// The protocol limits and the cross-field rules of the mode.
    pub fn validate(&self) -> Result<(), RejectionKind> {
        self.check_mode_fields()?;
        protocol_limits::check_title(&self.title)?;
        check_setting_range(self.world_width_pixels, SettingFieldKind::WorldSize)?;
        check_setting_range(self.world_height_pixels, SettingFieldKind::WorldSize)?;

        if let Some(player_minimum) = self.player_minimum {
            check_setting_range(u32::from(player_minimum), SettingFieldKind::PlayerMinimum)?;
        }

        check_setting_range(u32::from(self.player_cap), SettingFieldKind::PlayerCap)?;

        if let Some(team_count) = self.team_count {
            check_setting_range(u32::from(team_count), SettingFieldKind::TeamCount)?;
        }

        check_setting_range(u32::from(self.leaderboard_length), SettingFieldKind::LeaderboardLength)?;

        if self.player_minimum.is_some_and(|player_minimum| self.player_cap < player_minimum) {
            return Err(RejectionKind::PlayerCapBelowMinimum);
        }

        if self.team_count.is_some_and(|team_count| self.player_cap < team_count) {
            return Err(RejectionKind::PlayerCapBelowTeamCount);
        }

        Ok(())
    }

    /// A missing required field counts as below its range.
    fn check_mode_fields(&self) -> Result<(), RejectionKind> {
        let has_player_minimum: bool = self.player_minimum.is_some();
        let has_team_count: bool = self.team_count.is_some();

        match self.mode {
            GameModeKind::FreeForAll if has_player_minimum => Err(get_not_applicable(SettingFieldKind::PlayerMinimum)),
            GameModeKind::FreeForAll if has_team_count => Err(get_not_applicable(SettingFieldKind::TeamCount)),
            GameModeKind::Skirmish if has_player_minimum => Err(get_not_applicable(SettingFieldKind::PlayerMinimum)),
            GameModeKind::Skirmish if !has_team_count => Err(get_below_range(SettingFieldKind::TeamCount)),
            GameModeKind::Survival if has_team_count => Err(get_not_applicable(SettingFieldKind::TeamCount)),
            GameModeKind::Survival if !has_player_minimum => Err(get_below_range(SettingFieldKind::PlayerMinimum)),
            GameModeKind::FreeForAll | GameModeKind::Skirmish | GameModeKind::Survival => Ok(()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameModeKind {
    FreeForAll,
    Skirmish,
    Survival,
}

impl GameModeKind {
    pub fn code(self) -> &'static str {
        match self {
            GameModeKind::FreeForAll => FREE_FOR_ALL_CODE,
            GameModeKind::Skirmish => SKIRMISH_CODE,
            GameModeKind::Survival => SURVIVAL_CODE,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownGameModeCode {
    pub code: String,
}

impl TryFrom<&str> for GameModeKind {
    type Error = UnknownGameModeCode;

    fn try_from(code: &str) -> Result<GameModeKind, UnknownGameModeCode> {
        match code {
            FREE_FOR_ALL_CODE => Ok(GameModeKind::FreeForAll),
            SKIRMISH_CODE => Ok(GameModeKind::Skirmish),
            SURVIVAL_CODE => Ok(GameModeKind::Survival),
            _ => Err(UnknownGameModeCode {
                code: String::from(code),
            }),
        }
    }
}

fn check_setting_range(value: u32, field: SettingFieldKind) -> Result<(), RejectionKind> {
    if value < field.lowest() {
        return Err(get_below_range(field));
    }

    if value > field.highest() {
        return Err(RejectionKind::SettingOutOfRange {
            field,
            bound: RangeBoundKind::Above,
        });
    }

    Ok(())
}

fn get_below_range(field: SettingFieldKind) -> RejectionKind {
    RejectionKind::SettingOutOfRange {
        field,
        bound: RangeBoundKind::Below,
    }
}

fn get_not_applicable(field: SettingFieldKind) -> RejectionKind {
    RejectionKind::SettingNotApplicable { field }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;

    #[test]
    fn code_gives_the_three_letter_mode_codes() {
        assert_eq!(GameModeKind::FreeForAll.code(), "ffa");
        assert_eq!(GameModeKind::Skirmish.code(), "skm");
        assert_eq!(GameModeKind::Survival.code(), "srv");
    }

    #[test]
    fn try_from_reads_every_code() {
        for mode in [GameModeKind::FreeForAll, GameModeKind::Skirmish, GameModeKind::Survival] {
            assert_eq!(GameModeKind::try_from(mode.code()), Ok(mode));
        }
    }

    #[test]
    fn try_from_rejects_an_unknown_code() {
        assert_eq!(
            GameModeKind::try_from("ctf"),
            Err(UnknownGameModeCode {
                code: String::from("ctf"),
            }),
        );
    }

    fn create_settings(mode: GameModeKind) -> GameSettings {
        test_fixture::create_settings(mode, WorldShapeKind::Rectangle, 800)
    }

    fn get_out_of_range(field: SettingFieldKind, bound: RangeBoundKind) -> Result<(), RejectionKind> {
        Err(RejectionKind::SettingOutOfRange { field, bound })
    }

    #[test]
    fn validate_accepts_the_fixture_settings_of_every_mode() {
        for mode in [GameModeKind::FreeForAll, GameModeKind::Skirmish, GameModeKind::Survival] {
            assert_eq!(create_settings(mode).validate(), Ok(()));
        }
    }

    #[test]
    fn validate_rejects_a_field_the_mode_does_not_have() {
        let mut free_for_all_settings: GameSettings = create_settings(GameModeKind::FreeForAll);
        free_for_all_settings.team_count = Some(2);
        let mut skirmish_settings: GameSettings = create_settings(GameModeKind::Skirmish);
        skirmish_settings.player_minimum = Some(2);
        let mut survival_settings: GameSettings = create_settings(GameModeKind::Survival);
        survival_settings.team_count = Some(2);

        assert_eq!(
            free_for_all_settings.validate(),
            Err(RejectionKind::SettingNotApplicable {
                field: SettingFieldKind::TeamCount,
            }),
        );
        assert_eq!(
            skirmish_settings.validate(),
            Err(RejectionKind::SettingNotApplicable {
                field: SettingFieldKind::PlayerMinimum,
            }),
        );
        assert_eq!(
            survival_settings.validate(),
            Err(RejectionKind::SettingNotApplicable {
                field: SettingFieldKind::TeamCount,
            }),
        );
    }

    #[test]
    fn validate_counts_a_missing_required_field_as_below_range() {
        let mut skirmish_settings: GameSettings = create_settings(GameModeKind::Skirmish);
        skirmish_settings.team_count = None;
        let mut survival_settings: GameSettings = create_settings(GameModeKind::Survival);
        survival_settings.player_minimum = None;

        assert_eq!(
            skirmish_settings.validate(),
            get_out_of_range(SettingFieldKind::TeamCount, RangeBoundKind::Below),
        );
        assert_eq!(
            survival_settings.validate(),
            get_out_of_range(SettingFieldKind::PlayerMinimum, RangeBoundKind::Below),
        );
    }

    #[test]
    fn validate_checks_the_title() {
        let mut settings: GameSettings = create_settings(GameModeKind::FreeForAll);
        settings.title = String::new();

        assert_eq!(settings.validate(), Err(RejectionKind::TitleEmpty));
    }

    #[test]
    fn validate_accepts_world_sizes_at_the_limits_and_rejects_past_them() {
        let mut settings: GameSettings = create_settings(GameModeKind::FreeForAll);

        settings.world_width_pixels = 300;
        settings.world_height_pixels = 100_000;
        assert_eq!(settings.validate(), Ok(()));

        settings.world_width_pixels = 299;
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::WorldSize, RangeBoundKind::Below)
        );

        settings.world_width_pixels = 300;
        settings.world_height_pixels = 100_001;
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::WorldSize, RangeBoundKind::Above)
        );
    }

    #[test]
    fn validate_accepts_counts_at_the_limits_and_rejects_past_them() {
        let mut settings: GameSettings = create_settings(GameModeKind::Survival);
        settings.player_minimum = Some(2);
        settings.player_cap = 32;
        settings.leaderboard_length = 20;
        assert_eq!(settings.validate(), Ok(()));

        settings.player_minimum = Some(1);
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::PlayerMinimum, RangeBoundKind::Below)
        );

        settings.player_minimum = Some(33);
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::PlayerMinimum, RangeBoundKind::Above)
        );

        settings.player_minimum = Some(2);
        settings.player_cap = 33;
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::PlayerCap, RangeBoundKind::Above)
        );

        settings.player_cap = 1;
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::PlayerCap, RangeBoundKind::Below)
        );

        settings.player_cap = 2;
        settings.leaderboard_length = 0;
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::LeaderboardLength, RangeBoundKind::Below),
        );

        settings.leaderboard_length = 21;
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::LeaderboardLength, RangeBoundKind::Above),
        );
    }

    #[test]
    fn validate_accepts_team_counts_at_the_limits_and_rejects_past_them() {
        let mut settings: GameSettings = create_settings(GameModeKind::Skirmish);

        settings.team_count = Some(4);
        assert_eq!(settings.validate(), Ok(()));

        settings.team_count = Some(1);
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::TeamCount, RangeBoundKind::Below)
        );

        settings.team_count = Some(5);
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::TeamCount, RangeBoundKind::Above)
        );
    }

    #[test]
    fn validate_applies_the_cross_field_rules_of_the_mode_only() {
        let mut survival_settings: GameSettings = create_settings(GameModeKind::Survival);
        survival_settings.player_minimum = Some(5);
        survival_settings.player_cap = 4;
        let mut skirmish_settings: GameSettings = create_settings(GameModeKind::Skirmish);
        skirmish_settings.team_count = Some(4);
        skirmish_settings.player_cap = 3;
        let mut free_for_all_settings: GameSettings = create_settings(GameModeKind::FreeForAll);
        free_for_all_settings.player_cap = 2;

        assert_eq!(survival_settings.validate(), Err(RejectionKind::PlayerCapBelowMinimum));
        assert_eq!(
            skirmish_settings.validate(),
            Err(RejectionKind::PlayerCapBelowTeamCount)
        );
        assert_eq!(free_for_all_settings.validate(), Ok(()));
    }
}
