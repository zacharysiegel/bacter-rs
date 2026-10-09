use std::collections::BTreeMap;

use shared::ability::{AbilityPhase, FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use shared::game::{GameModeKind, GameSettings, GameState, Tick};
use shared::geometry::WorldPoint;
use shared::member::{Appearance, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind};
use shared::organism::Organism;
use shared::organism::{growth, spawn};
use shared::world::WorldShapeKind;

const WORLD_SIZE_PIXELS: u32 = 100_000;
const START_POSITION: WorldPoint = WorldPoint { x: 50_000, y: 50_000 };
const CURSOR_SPEED_MILLIPIXELS_PER_TICK: i64 = 2975;
const TICK_COUNT: u32 = 3000;
const DISCARDED_TICK_COUNT: u32 = 200;
const SEEDS: [u64; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
const CELL_COUNT_TOLERANCE_PROPORTION: f64 = 0.10;
const CHANGE_RATE_TOLERANCE_PROPORTION: f64 = 0.15;
const MEMBER_ID: MemberId = MemberId(0);

const REFERENCE_MEASUREMENTS: [ReferenceMeasurement; 6] = [
    ReferenceMeasurement {
        growth_state: ReferenceGrowthStateKind::Default,
        cursor_motion: CursorMotionKind::Still,
        mean_cell_count: 64.0,
        mean_changes_per_tick: 6.2,
    },
    ReferenceMeasurement {
        growth_state: ReferenceGrowthStateKind::Default,
        cursor_motion: CursorMotionKind::Moving,
        mean_cell_count: 42.5,
        mean_changes_per_tick: 7.7,
    },
    ReferenceMeasurement {
        growth_state: ReferenceGrowthStateKind::Compress,
        cursor_motion: CursorMotionKind::Still,
        mean_cell_count: 37.8,
        mean_changes_per_tick: 0.8,
    },
    ReferenceMeasurement {
        growth_state: ReferenceGrowthStateKind::Compress,
        cursor_motion: CursorMotionKind::Moving,
        mean_cell_count: 23.0,
        mean_changes_per_tick: 4.4,
    },
    ReferenceMeasurement {
        growth_state: ReferenceGrowthStateKind::Extend,
        cursor_motion: CursorMotionKind::Still,
        mean_cell_count: 124.8,
        mean_changes_per_tick: 7.8,
    },
    ReferenceMeasurement {
        growth_state: ReferenceGrowthStateKind::Extend,
        cursor_motion: CursorMotionKind::Moving,
        mean_cell_count: 81.2,
        mean_changes_per_tick: 10.4,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReferenceGrowthStateKind {
    Default,
    Compress,
    Extend,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CursorMotionKind {
    Still,
    Moving,
}

/// Means measured once from the original growth rules (`Org.js`, master).
struct ReferenceMeasurement {
    growth_state: ReferenceGrowthStateKind,
    cursor_motion: CursorMotionKind,
    mean_cell_count: f64,
    mean_changes_per_tick: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MeasuredMeans {
    mean_cell_count: f64,
    mean_changes_per_tick: f64,
}

#[test]
#[ignore = "long run; ./scripts/test/test-growth-statistics.sh"]
fn growth_statistics_match_the_original_rules() {
    let mut deviations: Vec<String> = Vec::new();

    for reference_measurement in &REFERENCE_MEASUREMENTS {
        let measured_means: MeasuredMeans =
            measure_means(reference_measurement.growth_state, reference_measurement.cursor_motion);
        println!(
            "{:?} {:?}: cells {:.1} (reference {:.1}), changes per tick {:.2} (reference {:.1})",
            reference_measurement.growth_state,
            reference_measurement.cursor_motion,
            measured_means.mean_cell_count,
            reference_measurement.mean_cell_count,
            measured_means.mean_changes_per_tick,
            reference_measurement.mean_changes_per_tick,
        );

        let is_cell_count_close: bool = is_within_tolerance(
            measured_means.mean_cell_count,
            reference_measurement.mean_cell_count,
            CELL_COUNT_TOLERANCE_PROPORTION,
        );
        let is_change_rate_close: bool = is_within_tolerance(
            measured_means.mean_changes_per_tick,
            reference_measurement.mean_changes_per_tick,
            CHANGE_RATE_TOLERANCE_PROPORTION,
        );
        if !is_cell_count_close || !is_change_rate_close {
            deviations.push(format!(
                "{:?} {:?}: {:?}",
                reference_measurement.growth_state, reference_measurement.cursor_motion, measured_means,
            ));
        }
    }

    assert_eq!(deviations, Vec::<String>::new());
}

fn measure_means(growth_state: ReferenceGrowthStateKind, cursor_motion: CursorMotionKind) -> MeasuredMeans {
    let mut cell_count_total: u64 = 0;
    let mut change_total: u64 = 0;
    let mut sample_count: u64 = 0;

    for seed in SEEDS {
        let mut state: GameState = create_state(seed);

        for tick_index in 0..TICK_COUNT {
            let cursor: WorldPoint = get_cursor(cursor_motion, tick_index);
            let changes: u32 = run_growth_tick(&mut state, growth_state, cursor);

            if tick_index >= DISCARDED_TICK_COUNT {
                cell_count_total += u64::from(get_organism(&state).cells.count());
                change_total += u64::from(changes);
                sample_count += 1;
            }
        }
    }

    MeasuredMeans {
        mean_cell_count: cell_count_total as f64 / sample_count as f64,
        mean_changes_per_tick: change_total as f64 / sample_count as f64,
    }
}

/// Births and natural deaths of one tick; an organism that dies is placed again as one cell at its cursor.
fn run_growth_tick(state: &mut GameState, growth_state: ReferenceGrowthStateKind, cursor: WorldPoint) -> u32 {
    let tick: Tick = state.tick.next();
    apply_growth_state(state.members.get_mut(&MEMBER_ID).unwrap(), growth_state);
    get_organism_mut(state).cursor = cursor;

    let cells_born: u32 = growth::run_birth_phase(state, tick);
    let cells_removed: u32 = growth::run_natural_death_phase(state);

    if get_organism(state).cells.is_empty() {
        spawn::place_organism(state, MEMBER_ID, cursor).unwrap();
    }

    get_organism_mut(state).cells.retighten();
    state.tick = tick;

    cells_born + cells_removed
}

fn get_cursor(cursor_motion: CursorMotionKind, tick_index: u32) -> WorldPoint {
    match cursor_motion {
        CursorMotionKind::Still => START_POSITION,
        CursorMotionKind::Moving => {
            let travelled_millipixels: i64 = CURSOR_SPEED_MILLIPIXELS_PER_TICK * (i64::from(tick_index) + 1);
            let travelled_pixels: i32 = i32::try_from((travelled_millipixels + 500) / 1000).unwrap();

            WorldPoint {
                x: START_POSITION.x + travelled_pixels,
                y: START_POSITION.y,
            }
        }
    }
}

fn apply_growth_state(member: &mut Member, growth_state: ReferenceGrowthStateKind) {
    let organism: &mut Organism = member.organism.as_mut().unwrap();

    match growth_state {
        ReferenceGrowthStateKind::Default => {}
        ReferenceGrowthStateKind::Compress => organism.abilities.compressed_until = Some(Tick(u32::MAX)),
        ReferenceGrowthStateKind::Extend => {
            organism.abilities.first = AbilityPhase::Active {
                ends_at: Tick(u32::MAX),
            }
        }
    }
}

fn create_state(seed: u64) -> GameState {
    let settings: GameSettings = GameSettings {
        title: String::from("Growth statistics"),
        mode: GameModeKind::FreeForAll,
        world_shape: WorldShapeKind::Rectangle,
        world_width_pixels: WORLD_SIZE_PIXELS,
        world_height_pixels: WORLD_SIZE_PIXELS,
        player_minimum: None,
        player_cap: 2,
        team_count: None,
        leaderboard_length: 10,
    };
    let member: Member = Member {
        member_id: MEMBER_ID,
        screen_name: String::from("grower"),
        role: MemberRoleKind::Participant,
        loadout: Some(Loadout {
            appearance: Appearance {
                color: OrganismColorKind::Leaf,
                skin: SkinKind::Grid,
            },
            first: FirstAbilityKind::Extend,
            second: SecondAbilityKind::Immortality,
            third: ThirdAbilityKind::Neutralize,
        }),
        team: None,
        score: Score::zero(),
        organism: Some(Organism::new(START_POSITION)),
    };

    let mut state: GameState = GameState::new(settings, seed);
    state.members = BTreeMap::from([(MEMBER_ID, member)]);
    state.next_member_id = MEMBER_ID.next();

    state
}

fn get_organism(state: &GameState) -> &Organism {
    state.members[&MEMBER_ID].organism.as_ref().unwrap()
}

fn get_organism_mut(state: &mut GameState) -> &mut Organism {
    state.members.get_mut(&MEMBER_ID).unwrap().organism.as_mut().unwrap()
}

fn is_within_tolerance(measured: f64, reference: f64, tolerance_proportion: f64) -> bool {
    (measured - reference).abs() <= reference * tolerance_proportion
}
