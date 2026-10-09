use crate::game::Tick;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoundState {
    pub phase: RoundPhase,
    pub phase_started_at: Tick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundPhase {
    Waiting,
    PreRound,
    Playing,
    PostRound,
}
