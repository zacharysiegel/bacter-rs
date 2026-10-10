use bitcode::{Decode, Encode};

use crate::error::AppError;
use crate::game::{GameId, GameModeKind, GameSummary};
use crate::protocol::protocol_limits;
use crate::protocol::{GameModeKindSerial, RejectionKind};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct GameSummarySerialOut {
    pub game_id: u32,
    pub title: String,
    pub mode: GameModeKindSerial,
    pub player_count: u8,
    pub spectator_count: u8,
    pub player_cap: u8,
    pub secured: bool,
    pub team_sizes: Vec<u8>,
}

impl From<&GameSummary> for GameSummarySerialOut {
    fn from(game_summary: &GameSummary) -> GameSummarySerialOut {
        GameSummarySerialOut {
            game_id: game_summary.game_id.0,
            title: game_summary.title.clone(),
            mode: GameModeKindSerial::from(&game_summary.mode),
            player_count: game_summary.player_count,
            spectator_count: game_summary.spectator_count,
            player_cap: game_summary.player_cap,
            secured: game_summary.secured,
            team_sizes: game_summary.team_sizes.clone(),
        }
    }
}

impl TryFrom<GameSummarySerialOut> for GameSummary {
    type Error = AppError;

    fn try_from(game_summary_serial_out: GameSummarySerialOut) -> Result<GameSummary, AppError> {
        protocol_limits::check_title(&game_summary_serial_out.title).map_err(RejectionKind::to_app_error)?;

        if game_summary_serial_out.team_sizes.len() > usize::from(protocol_limits::TEAM_COUNT_HIGHEST) {
            return Err(AppError::new("a game summary lists more teams than a game can have"));
        }

        Ok(GameSummary {
            game_id: GameId(game_summary_serial_out.game_id),
            title: game_summary_serial_out.title,
            mode: GameModeKind::from(game_summary_serial_out.mode),
            player_count: game_summary_serial_out.player_count,
            spectator_count: game_summary_serial_out.spectator_count,
            player_cap: game_summary_serial_out.player_cap,
            secured: game_summary_serial_out.secured,
            team_sizes: game_summary_serial_out.team_sizes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_game_summary() -> GameSummary {
        GameSummary {
            game_id: GameId(7),
            title: String::from("Arena"),
            mode: GameModeKind::Skirmish,
            player_count: 3,
            spectator_count: 2,
            player_cap: 8,
            secured: true,
            team_sizes: vec![2, 1],
        }
    }

    #[test]
    fn try_from_restores_the_game_summary() {
        let game_summary: GameSummary = create_game_summary();

        assert_eq!(
            GameSummary::try_from(GameSummarySerialOut::from(&game_summary)).unwrap(),
            game_summary
        );
    }

    #[test]
    fn try_from_rejects_an_invalid_title() {
        let mut game_summary_serial_out: GameSummarySerialOut = GameSummarySerialOut::from(&create_game_summary());
        game_summary_serial_out.title = String::new();

        assert!(GameSummary::try_from(game_summary_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_more_teams_than_a_game_can_have() {
        let mut game_summary_serial_out: GameSummarySerialOut = GameSummarySerialOut::from(&create_game_summary());
        game_summary_serial_out.team_sizes = vec![1; 5];

        assert!(GameSummary::try_from(game_summary_serial_out).is_err());
    }
}
