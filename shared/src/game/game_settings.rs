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

#[cfg(test)]
mod tests {
    use super::*;

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
}
