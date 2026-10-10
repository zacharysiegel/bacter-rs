use crate::game::GameSettings;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayHeader {
    pub settings: GameSettings,
    pub seed: u64,
}
