use crate::ability::{AbilityPressSet, AimVector};
use crate::game::Tick;
use crate::geometry::SubpixelPoint;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerInput {
    /// Tick of the last bundle the client applied when it sampled.
    pub client_tick: Tick,
    pub cursor: SubpixelPoint,
    pub ability_presses: AbilityPressSet,
    /// Present only with a first or second press.
    pub aim: Option<AimVector>,
}
