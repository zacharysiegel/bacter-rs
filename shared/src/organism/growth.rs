use std::collections::BTreeMap;

use crate::ability::{Loadout, OrganismAbilities};
use crate::game::{GameState, Tick};
use crate::geometry;
use crate::geometry::{LatticeCoordinate, WorldPoint};
use crate::member::{Member, MemberId};
use crate::organism::growth_chance_table;
use crate::organism::{GrowthChanceTable, GrowthStateKind};
use crate::random::Pcg32;
use crate::world::World;

struct CollisionIndex {
    collision_cells_by_bucket: BTreeMap<(i32, i32), Vec<CollisionCell>>,
}

impl CollisionIndex {
    fn from_members(members: &BTreeMap<MemberId, Member>) -> CollisionIndex {
        let mut collision_index: CollisionIndex = CollisionIndex {
            collision_cells_by_bucket: BTreeMap::new(),
        };

        for member in members.values() {
            let Some(organism) = &member.organism else {
                continue;
            };

            for lattice_coordinate in organism.cells.iter() {
                collision_index.insert(member.member_id, organism.cell_center(lattice_coordinate));
            }
        }

        collision_index
    }

    fn insert(&mut self, member_id: MemberId, center: WorldPoint) {
        let bucket: (i32, i32) = get_bucket(center);

        self.collision_cells_by_bucket.entry(bucket).or_default().push(CollisionCell { member_id, center });
    }

    /// Teammates collide too.
    fn collides_with_other_member(&self, member_id: MemberId, center: WorldPoint) -> bool {
        let (bucket_x, bucket_y): (i32, i32) = get_bucket(center);

        for neighbor_bucket_y in bucket_y - 1..=bucket_y + 1 {
            for neighbor_bucket_x in bucket_x - 1..=bucket_x + 1 {
                let Some(collision_cells) = self.collision_cells_by_bucket.get(&(neighbor_bucket_x, neighbor_bucket_y))
                else {
                    continue;
                };

                let collides: bool = collision_cells.iter().any(|collision_cell| {
                    collision_cell.member_id != member_id && collision_cell.center.is_within_cell_collision(center)
                });

                if collides {
                    return true;
                }
            }
        }

        false
    }
}

struct CollisionCell {
    member_id: MemberId,
    center: WorldPoint,
}

/// Returns the number of cells born.
pub fn run_birth_phase(state: &mut GameState, tick: Tick) -> u32 {
    let birth_order: Vec<MemberId> = get_birth_order(&state.members, tick);
    let mut collision_index: CollisionIndex = CollisionIndex::from_members(&state.members);
    let mut cells_born: u32 = 0;

    for member_id in birth_order {
        let Some(member) = state.members.get_mut(&member_id) else {
            continue;
        };

        cells_born += run_member_births(member, &state.world, &mut state.rng, &mut collision_index);
    }

    cells_born
}

/// Members with an organism in ascending id, rotated to start at index `tick % organism_count`.
pub fn get_birth_order(members: &BTreeMap<MemberId, Member>, tick: Tick) -> Vec<MemberId> {
    let mut birth_order: Vec<MemberId> =
        members.values().filter(|member| member.organism.is_some()).map(|member| member.member_id).collect();

    if birth_order.is_empty() {
        return birth_order;
    }

    let organism_count: u32 = u32::try_from(birth_order.len()).unwrap_or(u32::MAX);
    let start_index: usize = usize::try_from(tick.0 % organism_count).unwrap_or(0);
    birth_order.rotate_left(start_index);

    birth_order
}

/// Compressed XOR Extended selects that state; neither or both selects Default.
pub fn get_growth_state(abilities: &OrganismAbilities, loadout: Option<&Loadout>) -> GrowthStateKind {
    let is_compressed: bool = abilities.is_compressed();
    let is_extended: bool = loadout.is_some_and(|loadout| abilities.is_extended(loadout));

    match (is_compressed, is_extended) {
        (true, false) => GrowthStateKind::Compressed,
        (false, true) => GrowthStateKind::Extended,
        (false, false) | (true, true) => GrowthStateKind::Default,
    }
}

fn run_member_births(member: &mut Member, world: &World, rng: &mut Pcg32, collision_index: &mut CollisionIndex) -> u32 {
    let member_id: MemberId = member.member_id;
    let loadout: Option<&Loadout> = member.loadout.as_ref();
    let Some(organism) = member.organism.as_mut() else {
        return 0;
    };

    if organism.abilities.is_frozen() {
        return 0;
    }

    let growth_state: GrowthStateKind = get_growth_state(&organism.abilities, loadout);
    let chance_table: &GrowthChanceTable = growth_chance_table::GROWTH_CHANCE_TABLES.get(growth_state);
    let adjacent_sites: Vec<LatticeCoordinate> = organism.adjacent_sites();
    let mut cells_born: u32 = 0;

    for site in adjacent_sites {
        let center: WorldPoint = organism.cell_center(site);
        let is_blocked: bool =
            !world.contains_cell(center) || collision_index.collides_with_other_member(member_id, center);

        if is_blocked {
            continue;
        }

        let draw: u32 = rng.next_u32();
        let distance_squared: i64 = center.distance_squared(organism.cursor);
        let passes: bool = chance_table.birth_passes(distance_squared, draw);

        if !passes || organism.cells.contains(site) {
            continue;
        }

        organism.cells.insert(site);
        collision_index.insert(member_id, center);
        cells_born += 1;
    }

    cells_born
}

/// Returns the number of cells removed.
pub fn run_natural_death_phase(state: &mut GameState) -> u32 {
    let mut cells_removed: u32 = 0;

    for member in state.members.values_mut() {
        cells_removed += run_member_natural_deaths(member, &state.world, &mut state.rng);
    }

    cells_removed
}

fn run_member_natural_deaths(member: &mut Member, world: &World, rng: &mut Pcg32) -> u32 {
    let loadout: Option<&Loadout> = member.loadout.as_ref();
    let Some(organism) = member.organism.as_mut() else {
        return 0;
    };

    let is_immortal: bool = loadout.is_some_and(|loadout| organism.abilities.is_immortal(loadout));

    if organism.abilities.is_frozen() || is_immortal {
        return 0;
    }

    let growth_state: GrowthStateKind = get_growth_state(&organism.abilities, loadout);
    let chance_table: &GrowthChanceTable = growth_chance_table::GROWTH_CHANCE_TABLES.get(growth_state);
    let exposed_cells: Vec<LatticeCoordinate> = organism.exposed_cells();
    let mut cells_removed: u32 = 0;

    for lattice_coordinate in exposed_cells {
        let center: WorldPoint = organism.cell_center(lattice_coordinate);
        let distance_squared: i64 = center.distance_squared(organism.cursor);
        let is_death_forced: bool = distance_squared > growth_state.range_squared() || !world.contains_cell(center);

        if !is_death_forced {
            let draw: u32 = rng.next_u32();
            let passes: bool = chance_table.death_passes(distance_squared, draw);

            if !passes {
                continue;
            }
        }

        organism.cells.remove(lattice_coordinate);
        cells_removed += 1;
    }

    cells_removed
}

fn get_bucket(center: WorldPoint) -> (i32, i32) {
    (
        center.x.div_euclid(geometry::CELL_WIDTH_PIXELS),
        center.y.div_euclid(geometry::CELL_WIDTH_PIXELS),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{AbilityPhase, FirstAbilityKind};
    use crate::game::GameModeKind;
    use crate::game::test_fixture;
    use crate::member::TeamKind;
    use crate::organism::Organism;

    const FIRST_MEMBER_ID: MemberId = MemberId(0);
    const SECOND_MEMBER_ID: MemberId = MemberId(1);

    fn create_state_with_organisms(positions: &[WorldPoint]) -> GameState {
        test_fixture::create_state_with_organisms(GameModeKind::FreeForAll, 300, positions)
    }

    fn advance_rng(rng: &Pcg32, draw_count: u32) -> Pcg32 {
        let mut advanced_rng: Pcg32 = rng.clone();
        for _ in 0..draw_count {
            advanced_rng.next_u32();
        }

        advanced_rng
    }

    fn get_cells(state: &GameState, member_id: MemberId) -> Vec<LatticeCoordinate> {
        test_fixture::get_organism(state, member_id).cells.iter().collect()
    }

    #[test]
    fn get_growth_state_selects_by_exclusive_or() {
        let loadout: Loadout = test_fixture::create_loadout();
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();

        assert_eq!(get_growth_state(&abilities, Some(&loadout)), GrowthStateKind::Default);

        abilities.compressed_until = Some(Tick(30));
        assert_eq!(
            get_growth_state(&abilities, Some(&loadout)),
            GrowthStateKind::Compressed,
        );

        abilities.first = AbilityPhase::Active { ends_at: Tick(30) };
        assert_eq!(get_growth_state(&abilities, Some(&loadout)), GrowthStateKind::Default);

        abilities.compressed_until = None;
        assert_eq!(get_growth_state(&abilities, Some(&loadout)), GrowthStateKind::Extended);
    }

    #[test]
    fn get_growth_state_ignores_a_compress_casters_active_timer() {
        let mut loadout: Loadout = test_fixture::create_loadout();
        loadout.first = FirstAbilityKind::Compress;
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.first = AbilityPhase::Active { ends_at: Tick(30) };

        assert_eq!(get_growth_state(&abilities, Some(&loadout)), GrowthStateKind::Default);
    }

    #[test]
    fn get_birth_order_rotates_by_tick() {
        let mut state: GameState = create_state_with_organisms(&[
            WorldPoint { x: 50, y: 50 },
            WorldPoint { x: 150, y: 50 },
            WorldPoint { x: 250, y: 50 },
        ]);
        state.members.insert(MemberId(3), test_fixture::create_participant(MemberId(3)));

        assert_eq!(
            get_birth_order(&state.members, Tick(4)),
            vec![MemberId(1), MemberId(2), MemberId(0)],
        );
        assert_eq!(
            get_birth_order(&state.members, Tick(6)),
            vec![MemberId(0), MemberId(1), MemberId(2)],
        );
    }

    #[test]
    fn run_birth_phase_skips_a_frozen_organism() {
        let mut state: GameState = create_state_with_organisms(&[WorldPoint { x: 150, y: 150 }]);
        test_fixture::get_organism_mut(&mut state, FIRST_MEMBER_ID).abilities.frozen_until = Some(Tick(30));
        let rng_before: Pcg32 = state.rng.clone();

        assert_eq!(run_birth_phase(&mut state, Tick(1)), 0);
        assert_eq!(state.rng, rng_before);
        assert_eq!(
            get_cells(&state, FIRST_MEMBER_ID),
            vec![LatticeCoordinate { i: 0, j: 0 }],
        );
    }

    #[test]
    fn run_birth_phase_draws_once_per_site_in_order() {
        let mut state: GameState = create_state_with_organisms(&[WorldPoint { x: 150, y: 150 }]);
        let mut expected_rng: Pcg32 = state.rng.clone();
        let table: &GrowthChanceTable = growth_chance_table::GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);
        let organism: &Organism = test_fixture::get_organism(&state, FIRST_MEMBER_ID);
        let expected_births: Vec<LatticeCoordinate> = organism
            .adjacent_sites()
            .into_iter()
            .filter(|site| {
                let draw: u32 = expected_rng.next_u32();
                let distance_squared: i64 = organism.cell_center(*site).distance_squared(organism.cursor);

                table.birth_passes(distance_squared, draw)
            })
            .collect();

        let cells_born: u32 = run_birth_phase(&mut state, Tick(1));

        assert_eq!(state.rng, expected_rng);
        assert_eq!(cells_born, u32::try_from(expected_births.len()).unwrap());
        for site in expected_births {
            assert!(test_fixture::get_organism(&state, FIRST_MEMBER_ID).cells.contains(site));
        }
    }

    #[test]
    fn run_birth_phase_skips_sites_outside_the_world_without_a_draw() {
        let mut state: GameState = create_state_with_organisms(&[WorldPoint { x: 12, y: 150 }]);
        let expected_rng: Pcg32 = advance_rng(&state.rng, 3);

        run_birth_phase(&mut state, Tick(1));

        assert_eq!(state.rng, expected_rng);
        assert!(
            !test_fixture::get_organism(&state, FIRST_MEMBER_ID)
                .cells
                .contains(LatticeCoordinate { i: -1, j: 0 }),
        );
    }

    #[test]
    fn run_birth_phase_skips_sites_colliding_across_unaligned_lattices() {
        let mut state: GameState =
            create_state_with_organisms(&[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 112, y: 103 }]);
        let expected_rng: Pcg32 = advance_rng(&state.rng, 6);

        run_birth_phase(&mut state, Tick(1));

        assert_eq!(state.rng, expected_rng);
        assert!(!get_cells(&state, FIRST_MEMBER_ID).contains(&LatticeCoordinate { i: 1, j: 0 }));
        assert!(!get_cells(&state, SECOND_MEMBER_ID).contains(&LatticeCoordinate { i: -1, j: 0 }));
    }

    #[test]
    fn run_birth_phase_lets_teammates_block_each_other() {
        let mut state: GameState =
            create_state_with_organisms(&[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 112, y: 103 }]);
        for member in state.members.values_mut() {
            member.team = Some(TeamKind::Red);
        }
        let expected_rng: Pcg32 = advance_rng(&state.rng, 6);

        run_birth_phase(&mut state, Tick(1));

        assert_eq!(state.rng, expected_rng);
    }

    #[test]
    fn run_birth_phase_shows_earlier_births_to_later_organisms() {
        let mut even_tick_state: GameState =
            create_state_with_organisms(&[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 118, y: 100 }]);
        test_fixture::get_organism_mut(&mut even_tick_state, FIRST_MEMBER_ID).cursor = WorldPoint { x: 106, y: 100 };
        test_fixture::get_organism_mut(&mut even_tick_state, SECOND_MEMBER_ID).cursor = WorldPoint { x: 112, y: 100 };
        let mut odd_tick_state: GameState = even_tick_state.clone();

        run_birth_phase(&mut even_tick_state, Tick(2));
        run_birth_phase(&mut odd_tick_state, Tick(3));

        assert!(get_cells(&even_tick_state, FIRST_MEMBER_ID).contains(&LatticeCoordinate { i: 1, j: 0 }));
        assert!(!get_cells(&even_tick_state, SECOND_MEMBER_ID).contains(&LatticeCoordinate { i: -1, j: 0 }));
        assert!(!get_cells(&odd_tick_state, FIRST_MEMBER_ID).contains(&LatticeCoordinate { i: 1, j: 0 }));
        assert!(get_cells(&odd_tick_state, SECOND_MEMBER_ID).contains(&LatticeCoordinate { i: -1, j: 0 }));
    }

    #[test]
    fn run_birth_phase_rolls_a_shared_site_once_per_neighboring_cell() {
        let mut state: GameState = create_state_with_organisms(&[WorldPoint { x: 150, y: 150 }]);
        let organism: &mut Organism = test_fixture::get_organism_mut(&mut state, FIRST_MEMBER_ID);
        organism.cells.insert(LatticeCoordinate { i: 2, j: 0 });
        organism.cursor = WorldPoint { x: 156, y: 150 };
        let expected_rng: Pcg32 = advance_rng(&state.rng, 8);

        run_birth_phase(&mut state, Tick(1));

        assert_eq!(state.rng, expected_rng);
        assert!(get_cells(&state, FIRST_MEMBER_ID).contains(&LatticeCoordinate { i: 1, j: 0 }));
    }

    #[test]
    fn run_natural_death_phase_skips_frozen_and_immortal_organisms() {
        let far_cursor: WorldPoint = WorldPoint { x: 250, y: 250 };
        let mut state: GameState =
            create_state_with_organisms(&[WorldPoint { x: 50, y: 50 }, WorldPoint { x: 150, y: 50 }]);
        let frozen_organism: &mut Organism = test_fixture::get_organism_mut(&mut state, FIRST_MEMBER_ID);
        frozen_organism.cursor = far_cursor;
        frozen_organism.abilities.frozen_until = Some(Tick(30));

        let immortal_organism: &mut Organism = test_fixture::get_organism_mut(&mut state, SECOND_MEMBER_ID);
        immortal_organism.cursor = far_cursor;
        immortal_organism.abilities.second = AbilityPhase::Active { ends_at: Tick(30) };

        assert_eq!(run_natural_death_phase(&mut state), 0);
        assert_eq!(get_cells(&state, FIRST_MEMBER_ID).len(), 1);
        assert_eq!(get_cells(&state, SECOND_MEMBER_ID).len(), 1);
    }

    #[test]
    fn run_natural_death_phase_removes_cells_beyond_the_range_without_a_draw() {
        let mut state: GameState = create_state_with_organisms(&[WorldPoint { x: 100, y: 150 }]);
        test_fixture::get_organism_mut(&mut state, FIRST_MEMBER_ID).cursor = WorldPoint { x: 151, y: 150 };
        let rng_before: Pcg32 = state.rng.clone();

        assert_eq!(run_natural_death_phase(&mut state), 1);
        assert_eq!(state.rng, rng_before);
        assert!(test_fixture::get_organism(&state, FIRST_MEMBER_ID).cells.is_empty());
    }

    #[test]
    fn run_natural_death_phase_draws_within_the_range() {
        let mut state: GameState = create_state_with_organisms(&[WorldPoint { x: 100, y: 150 }]);
        test_fixture::get_organism_mut(&mut state, FIRST_MEMBER_ID).cursor = WorldPoint { x: 150, y: 150 };
        let expected_rng: Pcg32 = advance_rng(&state.rng, 1);

        assert_eq!(run_natural_death_phase(&mut state), 1);
        assert_eq!(state.rng, expected_rng);
    }

    #[test]
    fn run_natural_death_phase_extended_range_reaches_further() {
        let mut state: GameState = create_state_with_organisms(&[WorldPoint { x: 100, y: 150 }]);
        let organism: &mut Organism = test_fixture::get_organism_mut(&mut state, FIRST_MEMBER_ID);
        organism.cursor = WorldPoint { x: 160, y: 150 };
        organism.abilities.first = AbilityPhase::Active { ends_at: Tick(30) };
        let expected_rng: Pcg32 = advance_rng(&state.rng, 1);

        run_natural_death_phase(&mut state);

        assert_eq!(state.rng, expected_rng);
    }

    #[test]
    fn run_natural_death_phase_removes_cells_outside_the_world_without_a_draw() {
        let mut state: GameState = create_state_with_organisms(&[WorldPoint { x: 6, y: 150 }]);
        let rng_before: Pcg32 = state.rng.clone();

        assert_eq!(run_natural_death_phase(&mut state), 1);
        assert_eq!(state.rng, rng_before);
    }

    #[test]
    fn run_natural_death_phase_keeps_a_cell_at_the_cursor() {
        let mut state: GameState = create_state_with_organisms(&[WorldPoint { x: 150, y: 150 }]);
        let expected_rng: Pcg32 = advance_rng(&state.rng, 1);

        assert_eq!(run_natural_death_phase(&mut state), 0);
        assert_eq!(state.rng, expected_rng);
    }

    #[test]
    fn run_natural_death_phase_never_removes_enclosed_cells() {
        let mut state: GameState = create_state_with_organisms(&[WorldPoint { x: 100, y: 100 }]);
        let organism: &mut Organism = test_fixture::get_organism_mut(&mut state, FIRST_MEMBER_ID);

        for j in -1..=1 {
            for i in -1..=1 {
                organism.cells.insert(LatticeCoordinate { i, j });
            }
        }

        organism.cursor = WorldPoint { x: 250, y: 250 };

        assert_eq!(run_natural_death_phase(&mut state), 8);
        assert_eq!(
            get_cells(&state, FIRST_MEMBER_ID),
            vec![LatticeCoordinate { i: 0, j: 0 }],
        );
    }
}
