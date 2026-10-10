use bitcode::{Decode, Encode};

use crate::ability::{AbilityPhase, OrganismAbilities, Projectile, ShotPhase, SporePhase};
use crate::organism::{CellOccupancy, Organism};
use crate::protocol::{LatticeCoordinateSerialOut, SubpixelPointSerial, SubpixelVectorSerialOut, WorldPointSerialOut};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct OrganismSerialOut {
    pub anchor: WorldPointSerialOut,
    pub cells: CellOccupancySerialOut,
    pub cursor: WorldPointSerialOut,
    pub last_hitter: Option<u32>,
    pub abilities: OrganismAbilitiesSerialOut,
}

impl From<&Organism> for OrganismSerialOut {
    fn from(organism: &Organism) -> OrganismSerialOut {
        OrganismSerialOut {
            anchor: WorldPointSerialOut::from(&organism.anchor),
            cells: CellOccupancySerialOut::from(&organism.cells),
            cursor: WorldPointSerialOut::from(&organism.cursor),
            last_hitter: organism.last_hitter.map(|last_hitter| last_hitter.0),
            abilities: OrganismAbilitiesSerialOut::from(&organism.abilities),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct CellOccupancySerialOut {
    pub origin: LatticeCoordinateSerialOut,
    pub width: u16,
    pub height: u16,
    pub row_bitmap_bytes: Vec<u8>,
}

impl From<&CellOccupancy> for CellOccupancySerialOut {
    fn from(cell_occupancy: &CellOccupancy) -> CellOccupancySerialOut {
        CellOccupancySerialOut {
            origin: LatticeCoordinateSerialOut::from(&cell_occupancy.origin()),
            width: cell_occupancy.width(),
            height: cell_occupancy.height(),
            row_bitmap_bytes: cell_occupancy.row_bitmap_bytes().to_vec(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct OrganismAbilitiesSerialOut {
    pub first: AbilityPhaseSerialOut,
    pub second: AbilityPhaseSerialOut,
    pub third: AbilityPhaseSerialOut,
    pub third_center: Option<WorldPointSerialOut>,
    pub spore: SporePhaseSerialOut,
    pub shots: [ShotPhaseSerialOut; 2],
    pub compressed_until: Option<u32>,
    pub frozen_until: Option<u32>,
}

impl From<&OrganismAbilities> for OrganismAbilitiesSerialOut {
    fn from(abilities: &OrganismAbilities) -> OrganismAbilitiesSerialOut {
        OrganismAbilitiesSerialOut {
            first: AbilityPhaseSerialOut::from(&abilities.first),
            second: AbilityPhaseSerialOut::from(&abilities.second),
            third: AbilityPhaseSerialOut::from(&abilities.third),
            third_center: abilities.third_center.as_ref().map(WorldPointSerialOut::from),
            spore: SporePhaseSerialOut::from(&abilities.spore),
            shots: [
                ShotPhaseSerialOut::from(&abilities.shots[0]),
                ShotPhaseSerialOut::from(&abilities.shots[1]),
            ],
            compressed_until: abilities.compressed_until.map(|compressed_until| compressed_until.0),
            frozen_until: abilities.frozen_until.map(|frozen_until| frozen_until.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum AbilityPhaseSerialOut {
    Ready,
    Active { ends_at: u32 },
    Cooling { ready_at: u32 },
}

impl From<&AbilityPhase> for AbilityPhaseSerialOut {
    fn from(ability_phase: &AbilityPhase) -> AbilityPhaseSerialOut {
        match ability_phase {
            AbilityPhase::Ready => AbilityPhaseSerialOut::Ready,
            AbilityPhase::Active { ends_at } => AbilityPhaseSerialOut::Active { ends_at: ends_at.0 },
            AbilityPhase::Cooling { ready_at } => AbilityPhaseSerialOut::Cooling { ready_at: ready_at.0 },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SporePhaseSerialOut {
    Ready,
    Flying {
        ends_at: u32,
        spores: Vec<ProjectileSerialOut>,
    },
    Secreting {
        ends_at: u32,
        spores: Vec<ProjectileSerialOut>,
    },
    Cooling {
        ready_at: u32,
    },
}

impl From<&SporePhase> for SporePhaseSerialOut {
    fn from(spore_phase: &SporePhase) -> SporePhaseSerialOut {
        match spore_phase {
            SporePhase::Ready => SporePhaseSerialOut::Ready,
            SporePhase::Flying { ends_at, spores } => SporePhaseSerialOut::Flying {
                ends_at: ends_at.0,
                spores: spores.iter().map(ProjectileSerialOut::from).collect(),
            },
            SporePhase::Secreting { ends_at, spores } => SporePhaseSerialOut::Secreting {
                ends_at: ends_at.0,
                spores: spores.iter().map(ProjectileSerialOut::from).collect(),
            },
            SporePhase::Cooling { ready_at } => SporePhaseSerialOut::Cooling { ready_at: ready_at.0 },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ShotPhaseSerialOut {
    Ready,
    Flying { ends_at: u32, shot: ProjectileSerialOut },
    Secreting { ends_at: u32, center: SubpixelPointSerial },
    Cooling { ready_at: u32 },
}

impl From<&ShotPhase> for ShotPhaseSerialOut {
    fn from(shot_phase: &ShotPhase) -> ShotPhaseSerialOut {
        match shot_phase {
            ShotPhase::Ready => ShotPhaseSerialOut::Ready,
            ShotPhase::Flying { ends_at, shot } => ShotPhaseSerialOut::Flying {
                ends_at: ends_at.0,
                shot: ProjectileSerialOut::from(shot),
            },
            ShotPhase::Secreting { ends_at, center } => ShotPhaseSerialOut::Secreting {
                ends_at: ends_at.0,
                center: SubpixelPointSerial::from(center),
            },
            ShotPhase::Cooling { ready_at } => ShotPhaseSerialOut::Cooling { ready_at: ready_at.0 },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ProjectileSerialOut {
    pub position: SubpixelPointSerial,
    pub velocity: SubpixelVectorSerialOut,
}

impl From<&Projectile> for ProjectileSerialOut {
    fn from(projectile: &Projectile) -> ProjectileSerialOut {
        ProjectileSerialOut {
            position: SubpixelPointSerial::from(&projectile.position),
            velocity: SubpixelVectorSerialOut::from(&projectile.velocity),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Tick;
    use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, WorldPoint};
    use crate::member::MemberId;

    #[test]
    fn from_copies_the_cell_bitmap_parts() {
        let mut cell_occupancy: CellOccupancy = CellOccupancy::with_cell(LatticeCoordinate { i: -2, j: 3 });
        cell_occupancy.insert(LatticeCoordinate { i: 7, j: 4 });

        assert_eq!(
            CellOccupancySerialOut::from(&cell_occupancy),
            CellOccupancySerialOut {
                origin: LatticeCoordinateSerialOut { i: -2, j: 3 },
                width: 10,
                height: 2,
                row_bitmap_bytes: vec![0b0000_0001, 0b0000_0000, 0b0000_0000, 0b0000_0010],
            },
        );
    }

    #[test]
    fn from_copies_every_ability_phase() {
        let projectile: Projectile = Projectile {
            position: SubpixelPoint { x: 5, y: 6 },
            velocity: SubpixelVector { x: -1, y: 2 },
        };
        let mut organism: Organism = Organism::new(WorldPoint { x: 40, y: 50 });
        organism.last_hitter = Some(MemberId(3));
        organism.abilities = OrganismAbilities {
            first: AbilityPhase::Active { ends_at: Tick(9) },
            second: AbilityPhase::Cooling { ready_at: Tick(8) },
            third: AbilityPhase::Ready,
            third_center: Some(WorldPoint { x: 1, y: 1 }),
            spore: SporePhase::Secreting {
                ends_at: Tick(7),
                spores: vec![projectile],
            },
            shots: [
                ShotPhase::Flying {
                    ends_at: Tick(6),
                    shot: projectile,
                },
                ShotPhase::Secreting {
                    ends_at: Tick(5),
                    center: projectile.position,
                },
            ],
            compressed_until: Some(Tick(4)),
            frozen_until: None,
        };
        let projectile_serial_out: ProjectileSerialOut = ProjectileSerialOut {
            position: SubpixelPointSerial { x: 5, y: 6 },
            velocity: SubpixelVectorSerialOut { x: -1, y: 2 },
        };

        let organism_serial_out: OrganismSerialOut = OrganismSerialOut::from(&organism);

        assert_eq!(organism_serial_out.anchor, WorldPointSerialOut { x: 40, y: 50 });
        assert_eq!(organism_serial_out.last_hitter, Some(3));
        assert_eq!(
            organism_serial_out.abilities,
            OrganismAbilitiesSerialOut {
                first: AbilityPhaseSerialOut::Active { ends_at: 9 },
                second: AbilityPhaseSerialOut::Cooling { ready_at: 8 },
                third: AbilityPhaseSerialOut::Ready,
                third_center: Some(WorldPointSerialOut { x: 1, y: 1 }),
                spore: SporePhaseSerialOut::Secreting {
                    ends_at: 7,
                    spores: vec![projectile_serial_out],
                },
                shots: [
                    ShotPhaseSerialOut::Flying {
                        ends_at: 6,
                        shot: projectile_serial_out,
                    },
                    ShotPhaseSerialOut::Secreting {
                        ends_at: 5,
                        center: SubpixelPointSerial { x: 5, y: 6 },
                    },
                ],
                compressed_until: Some(4),
                frozen_until: None,
            },
        );
    }
}
