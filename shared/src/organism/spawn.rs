use std::collections::BTreeMap;

use crate::ability::ability_constants;
use crate::ability::{Loadout, ShotPhase, SporePhase};
use crate::game::{GameState, SimulationEvent, SpawnRejectionKind};
use crate::geometry;
use crate::geometry::{SubpixelPoint, WorldPoint};
use crate::member::{Member, MemberId, MemberRoleKind, TeamKind};
use crate::organism::Organism;
use crate::random::Pcg32;
use crate::round::RoundPhase;
use crate::world::{World, WorldBounds, WorldShapeKind};

pub const SPAWN_ATTEMPT_LIMIT: u32 = 64;
/// The original's 50 px buffer plus half a cell.
pub const SPAWN_MARGIN_PIXELS: i64 = 53;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementError {
    MemberNotFound { member_id: MemberId },
}

/// Candidate cursors are uniform in `[minimum, end)` on each axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SpawnRange {
    x_minimum: i64,
    x_end: i64,
    y_minimum: i64,
    y_end: i64,
}

/// `None` when the member does not exist.
pub fn spawn_member(
    state: &mut GameState,
    member_id: MemberId,
    loadout: Loadout,
    team: Option<TeamKind>,
) -> Option<SimulationEvent> {
    let member: &Member = state.members.get(&member_id)?;
    let rejection: Option<SpawnRejectionKind> = get_spawn_rejection(state, member);
    if let Some(reason) = rejection {
        return Some(SimulationEvent::SpawnRejected { member_id, reason });
    }

    let spawn_position: Option<WorldPoint> = find_spawn_position(&mut state.rng, &state.world, &state.members);

    let member: &mut Member = state.members.get_mut(&member_id)?;
    member.role = MemberRoleKind::Participant;
    member.loadout = Some(loadout.with_team_color(team));
    member.team = team;

    let Some(position) = spawn_position else {
        return Some(SimulationEvent::SpawnRejected {
            member_id,
            reason: SpawnRejectionKind::PositionNotFound,
        });
    };

    member.organism = Some(Organism::new(position));

    Some(SimulationEvent::OrganismSpawned {
        member_id,
        cursor: position,
    })
}

/// One cell at `position`, with no random draw and no hazard check.
pub fn place_organism(state: &mut GameState, member_id: MemberId, position: WorldPoint) -> Result<(), PlacementError> {
    let Some(member) = state.members.get_mut(&member_id) else {
        return Err(PlacementError::MemberNotFound { member_id });
    };

    member.organism = Some(Organism::new(position));

    Ok(())
}

/// Draws two values per candidate; `None` after `SPAWN_ATTEMPT_LIMIT` rejected candidates.
pub fn find_spawn_position(rng: &mut Pcg32, world: &World, members: &BTreeMap<MemberId, Member>) -> Option<WorldPoint> {
    let spawn_range: SpawnRange = get_spawn_range(&world.bounds)?;

    for _ in 0..SPAWN_ATTEMPT_LIMIT {
        let x: i64 = spawn_range.x_minimum + i64::from(rng.below(get_span(spawn_range.x_minimum, spawn_range.x_end)));
        let y: i64 = spawn_range.y_minimum + i64::from(rng.below(get_span(spawn_range.y_minimum, spawn_range.y_end)));
        let candidate: WorldPoint = WorldPoint {
            x: i32::try_from(x).ok()?,
            y: i32::try_from(y).ok()?,
        };

        if is_spawn_position_valid(world, members, candidate) {
            return Some(candidate);
        }
    }

    None
}

pub fn is_spawn_position_valid(world: &World, members: &BTreeMap<MemberId, Member>, candidate: WorldPoint) -> bool {
    let candidate_subpixels: SubpixelPoint = candidate.to_subpixel_point();
    let is_outside_ellipse: bool =
        world.shape == WorldShapeKind::Ellipse && world.bounds.is_outside_ellipse(candidate_subpixels);
    if is_outside_ellipse {
        return false;
    }

    members.values().all(|member| {
        let Some(organism) = &member.organism else {
            return true;
        };

        !collides_with_organism(organism, candidate) && !is_inside_hazard(member, organism, candidate)
    })
}

fn get_spawn_rejection(state: &GameState, member: &Member) -> Option<SpawnRejectionKind> {
    let is_round_in_progress: bool =
        state.round.is_some_and(|round| matches!(round.phase, RoundPhase::Playing | RoundPhase::PostRound));

    if member.organism.is_some() {
        return Some(SpawnRejectionKind::AlreadyAlive);
    }

    if state.alive_organism_count() >= u32::from(state.settings.player_cap) {
        return Some(SpawnRejectionKind::CapReached);
    }

    if is_round_in_progress {
        return Some(SpawnRejectionKind::RoundInProgress);
    }

    None
}

fn get_spawn_range(bounds: &WorldBounds) -> Option<SpawnRange> {
    let subpixels_per_pixel: i64 = i64::from(geometry::SUBPIXELS_PER_PIXEL);
    let left: i64 = i64::from(bounds.left.0);
    let top: i64 = i64::from(bounds.top.0);
    let left_pixels: i64 = -(-left).div_euclid(subpixels_per_pixel);
    let top_pixels: i64 = -(-top).div_euclid(subpixels_per_pixel);
    let right_pixels: i64 = (left + i64::from(bounds.width.0)).div_euclid(subpixels_per_pixel);
    let bottom_pixels: i64 = (top + i64::from(bounds.height.0)).div_euclid(subpixels_per_pixel);

    let spawn_range: SpawnRange = SpawnRange {
        x_minimum: left_pixels + SPAWN_MARGIN_PIXELS,
        x_end: right_pixels - SPAWN_MARGIN_PIXELS,
        y_minimum: top_pixels + SPAWN_MARGIN_PIXELS,
        y_end: bottom_pixels - SPAWN_MARGIN_PIXELS,
    };
    let is_empty: bool = spawn_range.x_end <= spawn_range.x_minimum || spawn_range.y_end <= spawn_range.y_minimum;
    if is_empty {
        return None;
    }

    Some(spawn_range)
}

fn get_span(minimum: i64, end: i64) -> u32 {
    u32::try_from(end - minimum).unwrap_or(u32::MAX)
}

fn collides_with_organism(organism: &Organism, candidate: WorldPoint) -> bool {
    organism
        .cells
        .iter()
        .any(|lattice_coordinate| organism.cell_center(lattice_coordinate).is_within_cell_collision(candidate))
}

fn is_inside_hazard(member: &Member, organism: &Organism, candidate: WorldPoint) -> bool {
    let candidate_subpixels: SubpixelPoint = candidate.to_subpixel_point();
    let is_inside_spore_secretion: bool = match &organism.abilities.spore {
        SporePhase::Secreting { spores, .. } => spores.iter().any(|spore| {
            spore.position.distance_squared(candidate_subpixels)
                <= ability_constants::SPORE_SECRETION_RADIUS_SQUARED_SUBPIXELS
        }),
        SporePhase::Ready | SporePhase::Flying { .. } | SporePhase::Cooling { .. } => false,
    };
    let is_inside_shot_secretion: bool = organism.abilities.shots.iter().any(|shot| match shot {
        ShotPhase::Secreting { center, .. } => {
            center.distance_squared(candidate_subpixels) <= ability_constants::SHOT_SECRETION_RADIUS_SQUARED_SUBPIXELS
        }
        ShotPhase::Ready | ShotPhase::Flying { .. } | ShotPhase::Cooling { .. } => false,
    });
    let is_toxin_field_active: bool =
        member.loadout.as_ref().is_some_and(|loadout| organism.abilities.is_toxin_field_active(loadout));
    let is_inside_toxin_field: bool = is_toxin_field_active
        && organism.abilities.third_center.is_some_and(|third_center| {
            third_center.distance_squared(candidate) <= ability_constants::FIELD_RADIUS_SQUARED_PIXELS
        });

    is_inside_spore_secretion || is_inside_shot_secretion || is_inside_toxin_field
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{AbilityPhase, Projectile, ThirdAbilityKind};
    use crate::game::{GameModeKind, Tick, test_fixture};
    use crate::geometry::{LatticeCoordinate, SubpixelVector};
    use crate::member::OrganismColorKind;
    use crate::round::RoundState;

    const SPAWNING_MEMBER_ID: MemberId = MemberId(0);
    const HAZARD_OWNER_ID: MemberId = MemberId(1);
    const HAZARD_CENTER: WorldPoint = WorldPoint { x: 150, y: 150 };

    fn create_state_with_hazard_owner() -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 300);
        state.members.insert(
            HAZARD_OWNER_ID,
            test_fixture::create_participant_with_organism(HAZARD_OWNER_ID, WorldPoint { x: 20, y: 20 }),
        );

        state
    }

    fn get_hazard_organism(state: &mut GameState) -> &mut Organism {
        test_fixture::get_organism_mut(state, HAZARD_OWNER_ID)
    }

    fn offset(point: WorldPoint, dx: i32) -> WorldPoint {
        WorldPoint {
            x: point.x + dx,
            y: point.y,
        }
    }

    fn create_covering_organism(member_id: MemberId) -> Member {
        let mut member: Member = test_fixture::create_participant_with_organism(member_id, WorldPoint { x: 53, y: 53 });
        let organism: &mut Organism = member.organism.as_mut().unwrap();

        for j in 0..=33 {
            for i in 0..=33 {
                organism.cells.insert(LatticeCoordinate { i, j });
            }
        }

        member
    }

    #[test]
    fn find_spawn_position_stays_inside_the_margin() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 300);
        let mut rng: Pcg32 = state.rng.clone();

        for _ in 0..500 {
            let position: WorldPoint = find_spawn_position(&mut rng, &state.world, &state.members).unwrap();

            assert!((53..247).contains(&position.x));
            assert!((53..247).contains(&position.y));
        }
    }

    #[test]
    fn find_spawn_position_gives_up_after_the_attempt_limit() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 300);
        state.members.insert(MemberId(0), create_covering_organism(MemberId(0)));
        let mut rng: Pcg32 = state.rng.clone();
        let mut expected_rng: Pcg32 = state.rng.clone();
        for _ in 0..2 * SPAWN_ATTEMPT_LIMIT {
            expected_rng.next_u32();
        }

        assert_eq!(find_spawn_position(&mut rng, &state.world, &state.members), None);
        assert_eq!(rng, expected_rng);
    }

    #[test]
    fn is_spawn_position_valid_rejects_cells_of_any_organism_inclusively() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 300);
        let mut member: Member =
            test_fixture::create_participant_with_organism(MemberId(0), WorldPoint { x: 100, y: 100 });
        member.organism.as_mut().unwrap().cells.insert(LatticeCoordinate { i: 1, j: 0 });
        state.members.insert(MemberId(0), member);

        assert!(!is_spawn_position_valid(
            &state.world,
            &state.members,
            WorldPoint { x: 112, y: 106 },
        ));
        assert!(is_spawn_position_valid(
            &state.world,
            &state.members,
            WorldPoint { x: 113, y: 106 },
        ));
    }

    #[test]
    fn is_spawn_position_valid_rejects_points_outside_an_ellipse() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Ellipse, 800);

        assert!(!is_spawn_position_valid(
            &state.world,
            &state.members,
            WorldPoint { x: 60, y: 60 },
        ));
        assert!(is_spawn_position_valid(
            &state.world,
            &state.members,
            WorldPoint { x: 400, y: 400 },
        ));
    }

    #[test]
    fn is_spawn_position_valid_rejects_spore_secretions() {
        let mut state: GameState = create_state_with_hazard_owner();
        get_hazard_organism(&mut state).abilities.spore = SporePhase::Secreting {
            ends_at: Tick(11),
            spores: vec![Projectile {
                position: HAZARD_CENTER.to_subpixel_point(),
                velocity: SubpixelVector { x: 0, y: 0 },
            }],
        };

        assert!(!is_spawn_position_valid(
            &state.world,
            &state.members,
            offset(HAZARD_CENTER, 24),
        ));
        assert!(is_spawn_position_valid(
            &state.world,
            &state.members,
            offset(HAZARD_CENTER, 25),
        ));
    }

    #[test]
    fn is_spawn_position_valid_rejects_shot_secretions() {
        let mut state: GameState = create_state_with_hazard_owner();
        get_hazard_organism(&mut state).abilities.shots[1] = ShotPhase::Secreting {
            ends_at: Tick(11),
            center: HAZARD_CENTER.to_subpixel_point(),
        };

        assert!(!is_spawn_position_valid(
            &state.world,
            &state.members,
            offset(HAZARD_CENTER, 12),
        ));
        assert!(is_spawn_position_valid(
            &state.world,
            &state.members,
            offset(HAZARD_CENTER, 13),
        ));
    }

    #[test]
    fn is_spawn_position_valid_rejects_active_toxin_fields_only() {
        let mut state: GameState = create_state_with_hazard_owner();
        let organism: &mut Organism = get_hazard_organism(&mut state);
        organism.abilities.third = AbilityPhase::Active { ends_at: Tick(57) };
        organism.abilities.third_center = Some(HAZARD_CENTER);

        assert!(is_spawn_position_valid(&state.world, &state.members, HAZARD_CENTER));

        state.members.get_mut(&HAZARD_OWNER_ID).unwrap().loadout.as_mut().unwrap().third = ThirdAbilityKind::Toxin;

        assert!(!is_spawn_position_valid(
            &state.world,
            &state.members,
            offset(HAZARD_CENTER, 60),
        ));
        assert!(is_spawn_position_valid(
            &state.world,
            &state.members,
            offset(HAZARD_CENTER, 61),
        ));
    }

    #[test]
    fn spawn_member_converts_a_spectator_and_spawns_one_cell() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);
        let mut spectator: Member = test_fixture::create_participant(SPAWNING_MEMBER_ID);
        spectator.role = MemberRoleKind::Spectator;
        spectator.loadout = None;
        state.members.insert(SPAWNING_MEMBER_ID, spectator);

        let simulation_event: Option<SimulationEvent> = spawn_member(
            &mut state,
            SPAWNING_MEMBER_ID,
            test_fixture::create_loadout(),
            Some(TeamKind::Green),
        );

        let member: &Member = &state.members[&SPAWNING_MEMBER_ID];
        let organism: &Organism = member.organism.as_ref().unwrap();
        assert_eq!(
            simulation_event,
            Some(SimulationEvent::OrganismSpawned {
                member_id: SPAWNING_MEMBER_ID,
                cursor: organism.cursor,
            }),
        );
        assert_eq!(member.role, MemberRoleKind::Participant);
        assert_eq!(member.team, Some(TeamKind::Green));
        assert_eq!(member.loadout.unwrap().appearance.color, OrganismColorKind::Lime);
        assert_eq!(organism.anchor, organism.cursor);
        assert_eq!(organism.cells.count(), 1);
    }

    #[test]
    fn spawn_member_rejects_an_alive_member() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.members.insert(
            SPAWNING_MEMBER_ID,
            test_fixture::create_participant_with_organism(SPAWNING_MEMBER_ID, WorldPoint { x: 100, y: 100 }),
        );

        assert_eq!(
            spawn_member(&mut state, SPAWNING_MEMBER_ID, test_fixture::create_loadout(), None),
            Some(SimulationEvent::SpawnRejected {
                member_id: SPAWNING_MEMBER_ID,
                reason: SpawnRejectionKind::AlreadyAlive,
            }),
        );
    }

    #[test]
    fn spawn_member_rejects_at_the_player_cap() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.settings.player_cap = 2;
        for (index, x) in [100, 300].iter().enumerate() {
            let member_id: MemberId = MemberId(u32::try_from(index).unwrap() + 1);
            state.members.insert(
                member_id,
                test_fixture::create_participant_with_organism(member_id, WorldPoint { x: *x, y: 100 }),
            );
        }
        state.members.insert(SPAWNING_MEMBER_ID, test_fixture::create_participant(SPAWNING_MEMBER_ID));

        assert_eq!(
            spawn_member(&mut state, SPAWNING_MEMBER_ID, test_fixture::create_loadout(), None),
            Some(SimulationEvent::SpawnRejected {
                member_id: SPAWNING_MEMBER_ID,
                reason: SpawnRejectionKind::CapReached,
            }),
        );
    }

    #[test]
    fn spawn_member_rejects_while_a_round_is_in_progress() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Rectangle, 800);
        state.round = Some(RoundState {
            phase: RoundPhase::Playing,
            phase_started_at: Tick(0),
        });
        state.members.insert(SPAWNING_MEMBER_ID, test_fixture::create_participant(SPAWNING_MEMBER_ID));

        assert_eq!(
            spawn_member(&mut state, SPAWNING_MEMBER_ID, test_fixture::create_loadout(), None),
            Some(SimulationEvent::SpawnRejected {
                member_id: SPAWNING_MEMBER_ID,
                reason: SpawnRejectionKind::RoundInProgress,
            }),
        );
    }

    #[test]
    fn spawn_member_reports_no_position_and_stays_without_an_organism() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 300);
        state.members.insert(HAZARD_OWNER_ID, create_covering_organism(HAZARD_OWNER_ID));
        state.members.insert(SPAWNING_MEMBER_ID, test_fixture::create_participant(SPAWNING_MEMBER_ID));

        assert_eq!(
            spawn_member(&mut state, SPAWNING_MEMBER_ID, test_fixture::create_loadout(), None),
            Some(SimulationEvent::SpawnRejected {
                member_id: SPAWNING_MEMBER_ID,
                reason: SpawnRejectionKind::PositionNotFound,
            }),
        );
        assert_eq!(state.members[&SPAWNING_MEMBER_ID].organism, None);
    }

    #[test]
    fn spawn_member_ignores_an_unknown_member() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);

        assert_eq!(
            spawn_member(&mut state, MemberId(9), test_fixture::create_loadout(), None),
            None,
        );
    }

    #[test]
    fn place_organism_places_one_cell_without_drawing() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.members.insert(SPAWNING_MEMBER_ID, test_fixture::create_participant(SPAWNING_MEMBER_ID));
        let rng_before: Pcg32 = state.rng.clone();

        assert_eq!(
            place_organism(&mut state, SPAWNING_MEMBER_ID, WorldPoint { x: 9, y: 9 }),
            Ok(()),
        );
        assert_eq!(state.rng, rng_before);
        assert_eq!(
            state.members[&SPAWNING_MEMBER_ID].organism,
            Some(Organism::new(WorldPoint { x: 9, y: 9 })),
        );
    }

    #[test]
    fn place_organism_rejects_an_unknown_member() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);

        assert_eq!(
            place_organism(&mut state, MemberId(3), WorldPoint { x: 9, y: 9 }),
            Err(PlacementError::MemberNotFound { member_id: MemberId(3) }),
        );
    }
}
