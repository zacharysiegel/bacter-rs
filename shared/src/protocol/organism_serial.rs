use bitcode::{Decode, Encode};

use crate::ability::{AbilityPhase, OrganismAbilities, Projectile, ShotPhase, SporePhase};
use crate::error::AppError;
use crate::game::Tick;
use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, WorldPoint};
use crate::member::MemberId;
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

impl TryFrom<OrganismSerialOut> for Organism {
    type Error = AppError;

    fn try_from(organism_serial_out: OrganismSerialOut) -> Result<Organism, AppError> {
        Ok(Organism {
            anchor: WorldPoint::try_from(organism_serial_out.anchor)?,
            cells: CellOccupancy::try_from(organism_serial_out.cells)?,
            cursor: WorldPoint::try_from(organism_serial_out.cursor)?,
            last_hitter: organism_serial_out.last_hitter.map(MemberId),
            abilities: OrganismAbilities::try_from(organism_serial_out.abilities)?,
        })
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

impl TryFrom<CellOccupancySerialOut> for CellOccupancy {
    type Error = AppError;

    fn try_from(cell_occupancy_serial_out: CellOccupancySerialOut) -> Result<CellOccupancy, AppError> {
        let origin: LatticeCoordinate = LatticeCoordinate::try_from(cell_occupancy_serial_out.origin)?;

        CellOccupancy::from_tight_bitmap(
            origin,
            cell_occupancy_serial_out.width,
            cell_occupancy_serial_out.height,
            cell_occupancy_serial_out.row_bitmap_bytes,
        )
        .ok_or_else(|| AppError::new("cell bitmap is not a tight bounding box of its stated size"))
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

impl TryFrom<OrganismAbilitiesSerialOut> for OrganismAbilities {
    type Error = AppError;

    fn try_from(abilities_serial_out: OrganismAbilitiesSerialOut) -> Result<OrganismAbilities, AppError> {
        let [first_shot_serial_out, second_shot_serial_out]: [ShotPhaseSerialOut; 2] = abilities_serial_out.shots;
        let third_center: Option<WorldPoint> =
            abilities_serial_out.third_center.map(WorldPoint::try_from).transpose()?;

        Ok(OrganismAbilities {
            first: AbilityPhase::from(abilities_serial_out.first),
            second: AbilityPhase::from(abilities_serial_out.second),
            third: AbilityPhase::from(abilities_serial_out.third),
            third_center,
            spore: SporePhase::try_from(abilities_serial_out.spore)?,
            shots: [
                ShotPhase::try_from(first_shot_serial_out)?,
                ShotPhase::try_from(second_shot_serial_out)?,
            ],
            compressed_until: abilities_serial_out.compressed_until.map(Tick),
            frozen_until: abilities_serial_out.frozen_until.map(Tick),
        })
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

impl From<AbilityPhaseSerialOut> for AbilityPhase {
    fn from(ability_phase_serial_out: AbilityPhaseSerialOut) -> AbilityPhase {
        match ability_phase_serial_out {
            AbilityPhaseSerialOut::Ready => AbilityPhase::Ready,
            AbilityPhaseSerialOut::Active { ends_at } => AbilityPhase::Active { ends_at: Tick(ends_at) },
            AbilityPhaseSerialOut::Cooling { ready_at } => AbilityPhase::Cooling {
                ready_at: Tick(ready_at),
            },
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

impl TryFrom<SporePhaseSerialOut> for SporePhase {
    type Error = AppError;

    fn try_from(spore_phase_serial_out: SporePhaseSerialOut) -> Result<SporePhase, AppError> {
        let spore_phase: SporePhase = match spore_phase_serial_out {
            SporePhaseSerialOut::Ready => SporePhase::Ready,
            SporePhaseSerialOut::Flying { ends_at, spores } => SporePhase::Flying {
                ends_at: Tick(ends_at),
                spores: convert_projectiles(spores)?,
            },
            SporePhaseSerialOut::Secreting { ends_at, spores } => SporePhase::Secreting {
                ends_at: Tick(ends_at),
                spores: convert_projectiles(spores)?,
            },
            SporePhaseSerialOut::Cooling { ready_at } => SporePhase::Cooling {
                ready_at: Tick(ready_at),
            },
        };

        Ok(spore_phase)
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

impl TryFrom<ShotPhaseSerialOut> for ShotPhase {
    type Error = AppError;

    fn try_from(shot_phase_serial_out: ShotPhaseSerialOut) -> Result<ShotPhase, AppError> {
        let shot_phase: ShotPhase = match shot_phase_serial_out {
            ShotPhaseSerialOut::Ready => ShotPhase::Ready,
            ShotPhaseSerialOut::Flying { ends_at, shot } => ShotPhase::Flying {
                ends_at: Tick(ends_at),
                shot: Projectile::try_from(shot)?,
            },
            ShotPhaseSerialOut::Secreting { ends_at, center } => ShotPhase::Secreting {
                ends_at: Tick(ends_at),
                center: SubpixelPoint::try_from(center)?,
            },
            ShotPhaseSerialOut::Cooling { ready_at } => ShotPhase::Cooling {
                ready_at: Tick(ready_at),
            },
        };

        Ok(shot_phase)
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

impl TryFrom<ProjectileSerialOut> for Projectile {
    type Error = AppError;

    fn try_from(projectile_serial_out: ProjectileSerialOut) -> Result<Projectile, AppError> {
        Ok(Projectile {
            position: SubpixelPoint::try_from(projectile_serial_out.position)?,
            velocity: SubpixelVector::try_from(projectile_serial_out.velocity)?,
        })
    }
}

fn convert_projectiles(projectile_serial_outs: Vec<ProjectileSerialOut>) -> Result<Vec<Projectile>, AppError> {
    projectile_serial_outs.into_iter().map(Projectile::try_from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;

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
        let organism: Organism = test_fixture::create_organism_with_projectiles();
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

    #[test]
    fn try_from_restores_the_organism() {
        let organism: Organism = test_fixture::create_organism_with_projectiles();

        assert_eq!(
            Organism::try_from(OrganismSerialOut::from(&organism)).unwrap(),
            organism
        );
    }

    #[test]
    fn try_from_restores_flying_spores_and_cooling_phases() {
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.spore = SporePhase::Flying {
            ends_at: Tick(30),
            spores: vec![test_fixture::PROJECTILE, test_fixture::PROJECTILE],
        };
        abilities.shots[1] = ShotPhase::Cooling { ready_at: Tick(31) };
        abilities.frozen_until = Some(Tick(32));

        assert_eq!(
            OrganismAbilities::try_from(OrganismAbilitiesSerialOut::from(&abilities)).unwrap(),
            abilities,
        );
    }

    #[test]
    fn try_from_rejects_a_cell_bitmap_of_the_wrong_length() {
        let cell_occupancy_serial_out: CellOccupancySerialOut = CellOccupancySerialOut {
            origin: LatticeCoordinateSerialOut { i: 0, j: 0 },
            width: 9,
            height: 1,
            row_bitmap_bytes: vec![0b0000_0001],
        };

        assert!(CellOccupancy::try_from(cell_occupancy_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_a_cell_bitmap_which_is_not_tight() {
        let cell_occupancy_serial_out: CellOccupancySerialOut = CellOccupancySerialOut {
            origin: LatticeCoordinateSerialOut { i: 0, j: 0 },
            width: 2,
            height: 1,
            row_bitmap_bytes: vec![0b0000_0001],
        };

        assert!(CellOccupancy::try_from(cell_occupancy_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_an_empty_cell_bitmap() {
        let cell_occupancy_serial_out: CellOccupancySerialOut = CellOccupancySerialOut::from(&CellOccupancy::empty());

        assert!(CellOccupancy::try_from(cell_occupancy_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_a_projectile_beyond_the_coordinate_limit() {
        let mut abilities_serial_out: OrganismAbilitiesSerialOut =
            OrganismAbilitiesSerialOut::from(&OrganismAbilities::all_ready());
        abilities_serial_out.shots[0] = ShotPhaseSerialOut::Secreting {
            ends_at: 3,
            center: SubpixelPointSerial { x: i32::MAX, y: 0 },
        };

        assert!(OrganismAbilities::try_from(abilities_serial_out).is_err());
    }
}
