use crate::ability::Loadout;
use crate::member::TeamKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Joiner {
    /// Trimmed.
    pub screen_name: String,
    pub loadout: Loadout,
    pub team: TeamChoiceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamChoiceKind {
    Auto,
    Team(TeamKind),
}
