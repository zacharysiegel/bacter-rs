use crate::ability::{AbilityPressSet, AimVector, Loadout};
use crate::game::Tick;
use crate::geometry::WorldPoint;
use crate::member::{Appearance, MemberId, MemberRoleKind, TeamKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputBundle {
    pub tick: Tick,
    /// Applied in this order.
    pub member_events: Vec<MemberEvent>,
    pub player_inputs: Vec<PlayerTickInput>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerTickInput {
    pub member_id: MemberId,
    pub cursor: WorldPoint,
    pub ability_presses: AbilityPressSet,
    pub aim: Option<AimVector>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemberEvent {
    Joined {
        member_id: MemberId,
        screen_name: String,
        role: MemberRoleKind,
        loadout: Option<Loadout>,
        team: Option<TeamKind>,
    },
    Left {
        member_id: MemberId,
    },
    SpawnRequested {
        member_id: MemberId,
        loadout: Loadout,
        team: Option<TeamKind>,
    },
    AppearanceChanged {
        member_id: MemberId,
        appearance: Appearance,
    },
}
