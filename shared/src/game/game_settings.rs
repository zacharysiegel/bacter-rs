use crate::world::WorldShapeKind;

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
            GameModeKind::FreeForAll => "ffa",
            GameModeKind::Skirmish => "skm",
            GameModeKind::Survival => "srv",
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
            "ffa" => Ok(GameModeKind::FreeForAll),
            "skm" => Ok(GameModeKind::Skirmish),
            "srv" => Ok(GameModeKind::Survival),
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
