use crate::game::GameModeKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSummary {
    pub game_id: GameId,
    pub title: String,
    pub mode: GameModeKind,
    /// Alive organisms.
    pub player_count: u8,
    /// Members without an organism.
    pub spectator_count: u8,
    pub player_cap: u8,
    pub secured: bool,
    /// Participants per team, in team order.
    pub team_sizes: Vec<u8>,
}
