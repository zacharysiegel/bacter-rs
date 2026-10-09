use crate::ability::ShotEffectKind;
use crate::geometry::WorldPoint;
use crate::member::MemberId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SimulationEvent {
    MemberJoined {
        member_id: MemberId,
    },
    MemberLeft {
        member_id: MemberId,
    },
    OrganismSpawned {
        member_id: MemberId,
        cursor: WorldPoint,
    },
    SpawnRejected {
        member_id: MemberId,
        reason: SpawnRejectionKind,
    },
    OrganismDied {
        member_id: MemberId,
        credited_to: Option<MemberId>,
    },
    EffectApplied {
        target: MemberId,
        caster: MemberId,
        kind: ShotEffectKind,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnRejectionKind {
    PositionNotFound,
    CapReached,
    RoundInProgress,
    AlreadyAlive,
}
