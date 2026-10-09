# Phase 2: simulation core and growth implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** the deterministic simulation core in `shared`: geometry, the hand-written `Pcg32`, the world and its border rule, organisms on a canonical cell bitmap, the growth chance tables, births and natural deaths, the spawn search and `place_organism`, member data, `GameSettings`, `GameState`, and `step` with the tick-order phases that exist so far (membership, cursors, births, deaths, death bookkeeping, re-tightening), plus the forbidden-operations scan and the growth statistics check against the original rules.

**Architecture:** every module follows `docs/architecture/overview.md` (module tree of `shared`, module rules) and `docs/architecture/simulation.md` (data model, tick order, growth rules, world and spawn); determinism follows `docs/architecture/determinism.md`. State is integers only, iteration is in the defined orders, and the only randomness is `GameState.rng`, drawn in the order the rules define. Each task adds one module with its inline unit tests, test first; `step` is assembled last from the phase functions.

**Tech stack:** Rust 1.95 (edition 2024), `libm` 0.2 (logarithm and square root of the chance tables), `std` only otherwise; no new dependencies.

---

## Ground rules for the executor

- Repository `/Users/singularity/bacter-rs`, branch `simulation-growth`. Run `git -C /Users/singularity/bacter-rs branch --show-current` first; stop if it does not print `simulation-growth`.
- Every command runs from the repository root `/Users/singularity/bacter-rs`.
- Never switch branches, push, rebase or amend. Stage by explicit path, run `git status` before each commit, one-line commit messages without attribution.
- Never run `cargo clippy`.
- The code below is already `rustfmt`-formatted with the repository's `rustfmt.toml`; transcribe it exactly, so `cargo fmt -p shared -- --check` stays silent.
- A test step creates a new file holding only its `#[cfg(test)] mod tests` block; the implementation step then writes the whole file (implementation above the same tests). Both versions of every such file are given in full.
- Builds stay warning-free; a warning is a defect to fix before committing.

## Decisions taken where the design is silent or contradictory

- Phase 3 modules whose types phase 2 types embed are created now as data only: `ability_model.rs` (loadout, kinds, phases, `Projectile`, `OrganismAbilities` with the predicates, `AbilityPressSet`, `AimVector`), `round.rs` (`RoundState`, `RoundPhase`), `scoreboard.rs` (`Score`), `simulation_event.rs` (the variants phase 2 emits), `ability_constants.rs` (the three hazard radii the spawn search needs). Activation, projectiles, damage, round transitions, leaderboard rows, the other events and constants stay in phase 3.
- `Tick` and `TICK_PERIOD_MILLISECONDS` live in `game/tick.rs`; the module tree names no file for them.
- `GameSettings` validation waits for phase 4 (`protocol_limits.rs`, `TryFrom<GameSettingsSerialIn>`); phase 2 has the types, `GameModeKind::code()` and `TryFrom<&str>` with error type `UnknownGameModeCode`.
- Birth rotation uses the tick being stepped (`bundle.tick`), passed to `growth::run_birth_phase`.
- Collision index entries carry the owning `MemberId` (`Vec<CollisionCell>` per bucket in place of `Vec<WorldPoint>`), so one index can skip each organism's own cells.
- Birth `distance_squared` past the table end: threshold 0 (never passes), draw still consumed.
- The growth phases return the cells born or removed; `step` ignores the counts. The statistics test drives the two phases directly, since its measure (births plus deaths, as the reference model) is not recoverable from cell sets after a step; a dead organism is placed again at its cursor with `place_organism`, as the reference model.
- Team colour forcing is applied in `step` (`Joined`, `SpawnRequested`, `AppearanceChanged`) through `Appearance::with_team_color` and `Loadout::with_team_color`; the design gives the rule, not its site.
- `spawn::spawn_member` checks `AlreadyAlive`, `CapReached`, `RoundInProgress` in that order without touching the member; then converts it (Participant, loadout, team) and searches, so `PositionNotFound` leaves a converted Participant without an organism.
- An empty whole-pixel spawn range (world narrower than 106 px) gives `PositionNotFound` without drawing.
- Events and inputs naming an unknown member are ignored without an event; `StepError` keeps the design's two kinds, `MemberIdOutOfOrder { minimum, received }`.
- `next_member_id` starts at `MemberId(0)`; `MemberId::next` saturates; `Tick::next` wraps.
- `PCG32_STREAM = 0xda3e_39cb_94b9_5bdb` (the PCG reference default increment constant). Reference vectors: published `pcg32_srandom_r(42, 54)` output. Golden sequence: `from_seed(0x0123_4567_89ab_cdef)`, eight draws, cross-checked with an independent implementation.
- Chance-table digest: FNV-1a 64 over every threshold as 8 little-endian bytes, Default, Compressed, Extended, birth table before death table; `0x75a8_9b54_1a01_86e5` with `libm` 0.2.16, matching an independent computation.
- `CellOccupancy::from_tight_bitmap` rejects an empty box (a canonical state never holds an organism without cells); `CellOccupancy::empty()` serves the transient state inside a tick.
- Enumeration is `Organism::exposed_cells` and `Organism::adjacent_sites`; `centroid` waits for its phase 3 consumers (spore and shot launch).
- `World::shrink` and `World::restore_initial_bounds` come with the world module; `step` calls them from phase 3 on.
- Forbidden-operations patterns: `.<name>(`, `f64::<name>(`, `f32::<name>(` for `ln`, `ln_1p`, `log`, `log2`, `log10`, `exp`, `exp2`, `exp_m1`, `powf`, `powi`, `sin`, `cos`, `tan`, `sin_cos`, `atan`, `atan2`, `hypot`, `cbrt`, `mul_add`, and `HashMap`, `HashSet`, outside `render/` and `play/`; `bitcode` outside `protocol/`. No `usize` scan (not among `testing.md`'s scan patterns).
- Growth statistics: cursor x `50_000 + round(2.975 * (t + 1))` px in integers (the reference moved the cursor before births), seeds 1 to 8, release build. Measured with this plan's code (cells, changes per tick): Default still 64.0, 6.22; moving 42.7, 7.78. Compress still 37.7, 0.79; moving 23.4, 4.45. Extend still 124.1, 7.97; moving 82.1, 10.40.
- Unit-test fixtures shared by `game_state`, `growth`, `spawn` and `step` live in `game/test_fixture.rs`, declared `#[cfg(test)]`.
- The natural death phase selects the growth state as the birth phase does.
- `spawn::place_organism` returns `Result<(), PlacementError>` (`PlacementError::MemberNotFound`); the design gives it no return type.
- `scripts/test/test-growth-statistics.sh` is created here although the roadmap lists the growth statistics script under phase 9: phase 2 is done only when the ignored statistics test passes, and ignored tests run through a `scripts/test/` script.

## File structure

- Create `shared/tests/forbidden_operations.rs`: source scan for forbidden float calls, hash collections and `bitcode` outside `protocol/`.
- Modify `shared/src/lib.rs`: declares the new top-level modules.
- Create `shared/src/geometry/mod.rs`, `shared/src/geometry/geometry.rs`: `WorldPoint`, `SubpixelPoint`, `SubpixelVector`, `Subpixels`, `LatticeCoordinate` (row-major order), `NeighborDirection`, cell size and subpixel constants, squared distances, the cell collision box.
- Create `shared/src/random/mod.rs`, `shared/src/random/pcg32.rs`: `Pcg32` (PCG32 XSH-RR): `from_seed`, `from_seed_and_stream`, `from_parts`, `next_u32`, `below`.
- Create `shared/src/organism/mod.rs`: organism module declarations and re-exports.
- Create `shared/src/organism/cell_occupancy.rs`: `CellOccupancy`: dense bitmap over a bounding box, insert, remove, row-major iteration, re-tightening, `from_tight_bitmap`.
- Create `shared/src/member/mod.rs`, `shared/src/member/member.rs`: `Member`, `MemberId`, `MemberRoleKind`, `Appearance`, `OrganismColorKind`, `SkinKind`, `TeamKind` and the team colours.
- Create `shared/src/member/scoreboard.rs`: `Score`.
- Create `shared/src/game/mod.rs`: game module declarations and re-exports, the test-only fixture module.
- Create `shared/src/game/tick.rs`: `Tick`, `TICK_PERIOD_MILLISECONDS`.
- Create `shared/src/ability/mod.rs`, `shared/src/ability/ability_model.rs`: `Loadout`, ability kinds, `AbilityPhase`, `SporePhase`, `ShotPhase`, `Projectile`, `OrganismAbilities` and the ability predicates, `AbilityPressSet`, `AimVector`.
- Create `shared/src/ability/ability_constants.rs`: spore secretion, shot secretion and field radii.
- Create `shared/src/organism/organism.rs`: `Organism`, exposed cells, adjacent sites with multiplicity.
- Create `shared/src/world/mod.rs`, `shared/src/world/world.rs`: `World`, `WorldShapeKind`, `WorldBounds`, the unified border rule `contains_cell`, the ellipse point test, survival shrink and restore.
- Create `shared/src/organism/growth_chance_table.rs`: `GrowthStateKind`, `GrowthChanceTable`, `GrowthChanceTables`, the static `GROWTH_CHANCE_TABLES`.
- Create `shared/src/game/game_settings.rs`: `GameSettings`, `GameModeKind` with `code()` and `TryFrom<&str>`.
- Create `shared/src/round/mod.rs`, `shared/src/round/round.rs`: `RoundState`, `RoundPhase`.
- Create `shared/src/game/game_state.rs`: `GameState`, `GameState::new`, alive organism count.
- Create `shared/src/game/test_fixture.rs`: unit-test fixtures: settings, states, loadouts, participants.
- Create `shared/src/organism/growth.rs`: growth state selection, birth order, the opponent collision index, the birth phase and the natural death phase.
- Create `shared/src/game/simulation_event.rs`: `SimulationEvent`, `SpawnRejectionKind`.
- Create `shared/src/organism/spawn.rs`: spawn eligibility, the spawn position search with hazards, `spawn_member`, `place_organism`.
- Create `shared/src/game/input_bundle.rs`: `InputBundle`, `PlayerTickInput`, `MemberEvent`.
- Create `shared/src/game/step.rs`: `step`, `StepError` and the phase-2 tick order.
- Create `shared/tests/growth_statistics.rs`: ignored long run comparing growth with the means measured from the original rules.
- Create `scripts/test/test-growth-statistics.sh`: runs the growth statistics test in release.

---

### Task 1: Forbidden-operations source scan

**Files:**
- Create: `shared/tests/forbidden_operations.rs`
- Test: `shared/tests/forbidden_operations.rs` (integration test)

The scan comes first so that every later module is checked as it lands.

- [ ] **Step 1: Write the scan test**

`shared/tests/forbidden_operations.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

const FLOAT_FUNCTION_NAMES: [&str; 19] = [
    "ln", "ln_1p", "log", "log2", "log10", "exp", "exp2", "exp_m1", "powf", "powi", "sin", "cos", "tan", "sin_cos",
    "atan", "atan2", "hypot", "cbrt", "mul_add",
];
const FORBIDDEN_COLLECTION_NAMES: [&str; 2] = ["HashMap", "HashSet"];
const DETERMINISM_EXEMPT_DIRECTORY_NAMES: [&str; 2] = ["render", "play"];
const BITCODE_PERMITTED_DIRECTORY_NAME: &str = "protocol";

#[derive(Debug)]
struct SourceFile {
    relative_path: PathBuf,
    contents: String,
}

impl SourceFile {
    fn is_inside_directory(&self, directory_name: &str) -> bool {
        self.relative_path.components().any(|component| component.as_os_str() == directory_name)
    }
}

#[test]
fn shared_source_uses_no_forbidden_operations() {
    let source_root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let source_files: Vec<SourceFile> = read_source_files(&source_root, &source_root);
    let violations: Vec<String> = source_files.iter().flat_map(find_violations).collect();

    assert_eq!(violations, Vec::<String>::new());
}

fn read_source_files(source_root: &Path, directory: &Path) -> Vec<SourceFile> {
    let mut source_files: Vec<SourceFile> = Vec::new();
    let mut entry_paths: Vec<PathBuf> = fs::read_dir(directory).unwrap().map(|entry| entry.unwrap().path()).collect();
    entry_paths.sort();

    for entry_path in entry_paths {
        let is_directory: bool = entry_path.is_dir();
        if is_directory {
            source_files.extend(read_source_files(source_root, &entry_path));
            continue;
        }

        let is_rust_file: bool = entry_path.extension().is_some_and(|extension| extension == "rs");
        if !is_rust_file {
            continue;
        }

        source_files.push(SourceFile {
            relative_path: entry_path.strip_prefix(source_root).unwrap().to_path_buf(),
            contents: fs::read_to_string(&entry_path).unwrap(),
        });
    }

    source_files
}

fn find_violations(source_file: &SourceFile) -> Vec<String> {
    let mut violations: Vec<String> = Vec::new();
    let is_determinism_exempt: bool = DETERMINISM_EXEMPT_DIRECTORY_NAMES
        .iter()
        .any(|directory_name| source_file.is_inside_directory(directory_name));

    if !is_determinism_exempt {
        violations.extend(find_pattern_violations(source_file, &get_determinism_patterns()));
    }

    if !source_file.is_inside_directory(BITCODE_PERMITTED_DIRECTORY_NAME) {
        violations.extend(find_pattern_violations(source_file, &[String::from("bitcode")]));
    }

    violations
}

fn get_determinism_patterns() -> Vec<String> {
    let mut patterns: Vec<String> = Vec::new();

    for function_name in FLOAT_FUNCTION_NAMES {
        patterns.push(format!(".{function_name}("));
        patterns.push(format!("f64::{function_name}("));
        patterns.push(format!("f32::{function_name}("));
    }

    for collection_name in FORBIDDEN_COLLECTION_NAMES {
        patterns.push(String::from(collection_name));
    }

    patterns
}

fn find_pattern_violations(source_file: &SourceFile, patterns: &[String]) -> Vec<String> {
    let mut violations: Vec<String> = Vec::new();

    for (line_index, line) in source_file.contents.lines().enumerate() {
        for pattern in patterns {
            if line.contains(pattern.as_str()) {
                violations.push(format!(
                    "{}:{}: {}",
                    source_file.relative_path.display(),
                    line_index + 1,
                    pattern,
                ));
            }
        }
    }

    violations
}
```

- [ ] **Step 2: Run it on the current tree**

Run: `cargo test -p shared --test forbidden_operations`
Expected: PASS, `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` (the crate has no code yet).

- [ ] **Step 3: Verify the scan detects violations**

Create the throwaway file `shared/src/forbidden_probe.rs` (it is not declared as a module; the scan reads files from disk):

```rust
use std::collections::HashMap;

pub fn probe(value: f64) -> f64 {
    value.ln()
}
```

Run: `cargo test -p shared --test forbidden_operations`
Expected: FAIL with

```text
  left: ["forbidden_probe.rs:1: HashMap", "forbidden_probe.rs:4: .ln("]
 right: []
```

- [ ] **Step 4: Remove the probe and rerun**

Run: `rm ./shared/src/forbidden_probe.rs && cargo test -p shared --test forbidden_operations`
Expected: PASS, `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/tests/forbidden_operations.rs
git commit -m "forbidden operations source scan"
```

`git status` before the commit lists only the paths staged above.

### Task 2: Geometry types

**Files:**
- Create: `shared/src/geometry/mod.rs`
- Create: `shared/src/geometry/geometry.rs`
- Modify: `shared/src/lib.rs`
- Test: inline `mod tests` in `shared/src/geometry/geometry.rs`

- [ ] **Step 1: Write the failing test**

`shared/src/lib.rs`, whole file:

```rust
//! Everything the server and the clients must agree on, plus platform-independent client logic.

pub mod geometry;
```

`shared/src/geometry/mod.rs`:

```rust
pub mod geometry;

pub use geometry::*;
```

`shared/src/geometry/geometry.rs`, tests only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lattice_coordinate_cmp_orders_rows_before_columns() {
        let mut lattice_coordinates: Vec<LatticeCoordinate> = vec![
            LatticeCoordinate { i: 0, j: 1 },
            LatticeCoordinate { i: 5, j: 0 },
            LatticeCoordinate { i: -3, j: 1 },
            LatticeCoordinate { i: 2, j: -1 },
        ];
        lattice_coordinates.sort();

        assert_eq!(
            lattice_coordinates,
            vec![
                LatticeCoordinate { i: 2, j: -1 },
                LatticeCoordinate { i: 5, j: 0 },
                LatticeCoordinate { i: -3, j: 1 },
                LatticeCoordinate { i: 0, j: 1 },
            ],
        );
    }

    #[test]
    fn neighbor_follows_left_up_right_down() {
        let origin: LatticeCoordinate = LatticeCoordinate { i: 0, j: 0 };
        let neighbors: Vec<LatticeCoordinate> =
            NEIGHBOR_DIRECTIONS.iter().map(|direction| origin.neighbor(*direction)).collect();

        assert_eq!(
            neighbors,
            vec![
                LatticeCoordinate { i: -1, j: 0 },
                LatticeCoordinate { i: 0, j: -1 },
                LatticeCoordinate { i: 1, j: 0 },
                LatticeCoordinate { i: 0, j: 1 },
            ],
        );
    }

    #[test]
    fn to_cell_center_scales_by_cell_width_from_anchor() {
        let anchor: WorldPoint = WorldPoint { x: 100, y: 200 };

        assert_eq!(
            LatticeCoordinate { i: 2, j: -3 }.to_cell_center(anchor),
            WorldPoint { x: 112, y: 182 },
        );
    }

    #[test]
    fn distance_squared_is_exact_at_coordinate_limits() {
        let first: SubpixelPoint = SubpixelPoint {
            x: -(1 << 28),
            y: -(1 << 28),
        };
        let second: SubpixelPoint = SubpixelPoint { x: 1 << 28, y: 1 << 28 };

        assert_eq!(first.distance_squared(second), 1_i64 << 59);
        assert_eq!(
            WorldPoint { x: 3, y: -4 }.distance_squared(WorldPoint { x: 0, y: 0 }),
            25,
        );
    }

    #[test]
    fn is_within_cell_collision_includes_touching_edges() {
        let center: WorldPoint = WorldPoint { x: 100, y: 100 };

        assert!(center.is_within_cell_collision(WorldPoint { x: 106, y: 94 }));
        assert!(!center.is_within_cell_collision(WorldPoint { x: 107, y: 100 }));
        assert!(!center.is_within_cell_collision(WorldPoint { x: 100, y: 93 }));
    }

    #[test]
    fn to_subpixel_point_multiplies_by_subpixels_per_pixel() {
        assert_eq!(
            WorldPoint { x: 3, y: -2 }.to_subpixel_point(),
            SubpixelPoint { x: 3072, y: -2048 },
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared geometry`
Expected: FAIL, compilation stops; the first error is `` error[E0425]: cannot find type `LatticeCoordinate` in this scope ``.

- [ ] **Step 3: Write the implementation**

Replace the whole of `shared/src/geometry/geometry.rs` with:

```rust
use std::cmp::Ordering;

pub const CELL_WIDTH_PIXELS: i32 = 6;
pub const SUBPIXELS_PER_PIXEL: i32 = 1024;
pub const NEIGHBOR_DIRECTIONS: [NeighborDirection; 4] = [
    NeighborDirection::Left,
    NeighborDirection::Up,
    NeighborDirection::Right,
    NeighborDirection::Down,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Subpixels(pub i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldPoint {
    pub x: i32,
    pub y: i32,
}

impl WorldPoint {
    pub fn distance_squared(self, other: WorldPoint) -> i64 {
        let dx: i64 = i64::from(self.x) - i64::from(other.x);
        let dy: i64 = i64::from(self.y) - i64::from(other.y);

        dx * dx + dy * dy
    }

    /// Whether two cell centres are close enough to collide, touching edges included.
    pub fn is_within_cell_collision(self, other: WorldPoint) -> bool {
        let dx: i64 = i64::from(self.x) - i64::from(other.x);
        let dy: i64 = i64::from(self.y) - i64::from(other.y);
        let collision_extent: i64 = i64::from(CELL_WIDTH_PIXELS);

        dx.abs() <= collision_extent && dy.abs() <= collision_extent
    }

    pub fn to_subpixel_point(self) -> SubpixelPoint {
        SubpixelPoint {
            x: self.x * SUBPIXELS_PER_PIXEL,
            y: self.y * SUBPIXELS_PER_PIXEL,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubpixelPoint {
    pub x: i32,
    pub y: i32,
}

impl SubpixelPoint {
    pub fn distance_squared(self, other: SubpixelPoint) -> i64 {
        let dx: i64 = i64::from(self.x) - i64::from(other.x);
        let dy: i64 = i64::from(self.y) - i64::from(other.y);

        dx * dx + dy * dy
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubpixelVector {
    pub x: i32,
    pub y: i32,
}

/// Ordered row-major: by `j`, then by `i`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LatticeCoordinate {
    pub i: i32,
    pub j: i32,
}

impl LatticeCoordinate {
    pub fn neighbor(self, direction: NeighborDirection) -> LatticeCoordinate {
        let (offset_i, offset_j): (i32, i32) = direction.offset();

        LatticeCoordinate {
            i: self.i + offset_i,
            j: self.j + offset_j,
        }
    }

    pub fn to_cell_center(self, anchor: WorldPoint) -> WorldPoint {
        WorldPoint {
            x: anchor.x + CELL_WIDTH_PIXELS * self.i,
            y: anchor.y + CELL_WIDTH_PIXELS * self.j,
        }
    }
}

impl Ord for LatticeCoordinate {
    fn cmp(&self, other: &LatticeCoordinate) -> Ordering {
        self.j.cmp(&other.j).then(self.i.cmp(&other.i))
    }
}

impl PartialOrd for LatticeCoordinate {
    fn partial_cmp(&self, other: &LatticeCoordinate) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeighborDirection {
    Left,
    Up,
    Right,
    Down,
}

impl NeighborDirection {
    pub fn offset(self) -> (i32, i32) {
        match self {
            NeighborDirection::Left => (-1, 0),
            NeighborDirection::Up => (0, -1),
            NeighborDirection::Right => (1, 0),
            NeighborDirection::Down => (0, 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lattice_coordinate_cmp_orders_rows_before_columns() {
        let mut lattice_coordinates: Vec<LatticeCoordinate> = vec![
            LatticeCoordinate { i: 0, j: 1 },
            LatticeCoordinate { i: 5, j: 0 },
            LatticeCoordinate { i: -3, j: 1 },
            LatticeCoordinate { i: 2, j: -1 },
        ];
        lattice_coordinates.sort();

        assert_eq!(
            lattice_coordinates,
            vec![
                LatticeCoordinate { i: 2, j: -1 },
                LatticeCoordinate { i: 5, j: 0 },
                LatticeCoordinate { i: -3, j: 1 },
                LatticeCoordinate { i: 0, j: 1 },
            ],
        );
    }

    #[test]
    fn neighbor_follows_left_up_right_down() {
        let origin: LatticeCoordinate = LatticeCoordinate { i: 0, j: 0 };
        let neighbors: Vec<LatticeCoordinate> =
            NEIGHBOR_DIRECTIONS.iter().map(|direction| origin.neighbor(*direction)).collect();

        assert_eq!(
            neighbors,
            vec![
                LatticeCoordinate { i: -1, j: 0 },
                LatticeCoordinate { i: 0, j: -1 },
                LatticeCoordinate { i: 1, j: 0 },
                LatticeCoordinate { i: 0, j: 1 },
            ],
        );
    }

    #[test]
    fn to_cell_center_scales_by_cell_width_from_anchor() {
        let anchor: WorldPoint = WorldPoint { x: 100, y: 200 };

        assert_eq!(
            LatticeCoordinate { i: 2, j: -3 }.to_cell_center(anchor),
            WorldPoint { x: 112, y: 182 },
        );
    }

    #[test]
    fn distance_squared_is_exact_at_coordinate_limits() {
        let first: SubpixelPoint = SubpixelPoint {
            x: -(1 << 28),
            y: -(1 << 28),
        };
        let second: SubpixelPoint = SubpixelPoint { x: 1 << 28, y: 1 << 28 };

        assert_eq!(first.distance_squared(second), 1_i64 << 59);
        assert_eq!(
            WorldPoint { x: 3, y: -4 }.distance_squared(WorldPoint { x: 0, y: 0 }),
            25,
        );
    }

    #[test]
    fn is_within_cell_collision_includes_touching_edges() {
        let center: WorldPoint = WorldPoint { x: 100, y: 100 };

        assert!(center.is_within_cell_collision(WorldPoint { x: 106, y: 94 }));
        assert!(!center.is_within_cell_collision(WorldPoint { x: 107, y: 100 }));
        assert!(!center.is_within_cell_collision(WorldPoint { x: 100, y: 93 }));
    }

    #[test]
    fn to_subpixel_point_multiplies_by_subpixels_per_pixel() {
        assert_eq!(
            WorldPoint { x: 3, y: -2 }.to_subpixel_point(),
            SubpixelPoint { x: 3072, y: -2048 },
        );
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared geometry`
Expected: the `shared` library tests report `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/src/geometry/geometry.rs ./shared/src/geometry/mod.rs ./shared/src/lib.rs
git commit -m "geometry types"
```

`git status` before the commit lists only the paths staged above.

### Task 3: Pcg32

**Files:**
- Create: `shared/src/random/mod.rs`
- Create: `shared/src/random/pcg32.rs`
- Modify: `shared/src/lib.rs`
- Test: inline `mod tests` in `shared/src/random/pcg32.rs`

The reference vectors are the published output of the PCG reference `pcg32_srandom_r(42, 54)`; `below(100)` of the first two draws is `(0xa15c02b7 * 100) >> 32 = 63` and `(0x7b47f409 * 100) >> 32 = 48`.

- [ ] **Step 1: Write the failing test**

`shared/src/lib.rs`, whole file:

```rust
//! Everything the server and the clients must agree on, plus platform-independent client logic.

pub mod geometry;
pub mod random;
```

`shared/src/random/mod.rs`:

```rust
pub mod pcg32;

pub use pcg32::*;
```

`shared/src/random/pcg32.rs`, tests only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_seed_and_stream_matches_reference_vectors() {
        let mut generator: Pcg32 = Pcg32::from_seed_and_stream(42, 54);
        let draws: Vec<u32> = (0..6).map(|_| generator.next_u32()).collect();

        assert_eq!(
            draws,
            vec![0xa15c02b7, 0x7b47f409, 0xba1d3330, 0x83d2f293, 0xbfa4784b, 0xcbed606e],
        );
    }

    #[test]
    fn from_seed_matches_golden_sequence() {
        let mut generator: Pcg32 = Pcg32::from_seed(0x0123_4567_89ab_cdef);
        let draws: Vec<u32> = (0..8).map(|_| generator.next_u32()).collect();

        assert_eq!(
            draws,
            vec![
                0x683ae4b0, 0x465b94e2, 0x8e78504b, 0x716c5c5d, 0x086b6029, 0x1aa1bf3a, 0x32af10fa, 0xa01c2abc,
            ],
        );
    }

    #[test]
    fn below_takes_the_high_word_of_the_scaled_draw() {
        let mut generator: Pcg32 = Pcg32::from_seed_and_stream(42, 54);

        assert_eq!(generator.below(100), 63);
        assert_eq!(generator.below(100), 48);
        assert_eq!(generator.below(1), 0);
    }

    #[test]
    fn from_parts_rejects_even_increment() {
        assert_eq!(Pcg32::from_parts(7, 10), None);
    }

    #[test]
    fn from_parts_restores_a_generator_mid_sequence() {
        let mut generator: Pcg32 = Pcg32::from_seed(99);
        generator.next_u32();

        let mut restored: Pcg32 = Pcg32::from_parts(generator.state(), generator.increment()).unwrap();

        assert_eq!(restored.next_u32(), generator.next_u32());
        assert_eq!(restored, generator);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared pcg32`
Expected: FAIL, compilation stops; the first error is `` error[E0425]: cannot find type `Pcg32` in this scope ``.

- [ ] **Step 3: Write the implementation**

Replace the whole of `shared/src/random/pcg32.rs` with:

```rust
pub const PCG32_STREAM: u64 = 0xda3e_39cb_94b9_5bdb;
const PCG32_MULTIPLIER: u64 = 6_364_136_223_846_793_005;

/// PCG32 XSH-RR.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pcg32 {
    state: u64,
    increment: u64,
}

impl Pcg32 {
    pub fn from_seed(seed: u64) -> Pcg32 {
        Pcg32::from_seed_and_stream(seed, PCG32_STREAM)
    }

    /// The reference `pcg32_srandom_r(initial_state, stream)`.
    pub fn from_seed_and_stream(initial_state: u64, stream: u64) -> Pcg32 {
        let mut generator: Pcg32 = Pcg32 {
            state: 0,
            increment: (stream << 1) | 1,
        };

        generator.advance();
        generator.state = generator.state.wrapping_add(initial_state);
        generator.advance();

        generator
    }

    /// `None` when `increment` is even.
    pub fn from_parts(state: u64, increment: u64) -> Option<Pcg32> {
        if increment % 2 == 0 {
            return None;
        }

        Some(Pcg32 { state, increment })
    }

    pub fn state(&self) -> u64 {
        self.state
    }

    pub fn increment(&self) -> u64 {
        self.increment
    }

    pub fn next_u32(&mut self) -> u32 {
        let previous_state: u64 = self.state;
        self.advance();

        // Truncation to the low 32 bits is the XSH step of the reference output function.
        let xor_shifted: u32 = (((previous_state >> 18) ^ previous_state) >> 27) as u32;
        let rotation: u32 = (previous_state >> 59) as u32;

        xor_shifted.rotate_right(rotation)
    }

    /// Uniform in `[0, bound)` by multiply-high; `bound` 0 gives 0.
    pub fn below(&mut self, bound: u32) -> u32 {
        let draw: u64 = u64::from(self.next_u32());

        ((draw * u64::from(bound)) >> 32) as u32
    }

    fn advance(&mut self) {
        self.state = self.state.wrapping_mul(PCG32_MULTIPLIER).wrapping_add(self.increment);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_seed_and_stream_matches_reference_vectors() {
        let mut generator: Pcg32 = Pcg32::from_seed_and_stream(42, 54);
        let draws: Vec<u32> = (0..6).map(|_| generator.next_u32()).collect();

        assert_eq!(
            draws,
            vec![0xa15c02b7, 0x7b47f409, 0xba1d3330, 0x83d2f293, 0xbfa4784b, 0xcbed606e],
        );
    }

    #[test]
    fn from_seed_matches_golden_sequence() {
        let mut generator: Pcg32 = Pcg32::from_seed(0x0123_4567_89ab_cdef);
        let draws: Vec<u32> = (0..8).map(|_| generator.next_u32()).collect();

        assert_eq!(
            draws,
            vec![
                0x683ae4b0, 0x465b94e2, 0x8e78504b, 0x716c5c5d, 0x086b6029, 0x1aa1bf3a, 0x32af10fa, 0xa01c2abc,
            ],
        );
    }

    #[test]
    fn below_takes_the_high_word_of_the_scaled_draw() {
        let mut generator: Pcg32 = Pcg32::from_seed_and_stream(42, 54);

        assert_eq!(generator.below(100), 63);
        assert_eq!(generator.below(100), 48);
        assert_eq!(generator.below(1), 0);
    }

    #[test]
    fn from_parts_rejects_even_increment() {
        assert_eq!(Pcg32::from_parts(7, 10), None);
    }

    #[test]
    fn from_parts_restores_a_generator_mid_sequence() {
        let mut generator: Pcg32 = Pcg32::from_seed(99);
        generator.next_u32();

        let mut restored: Pcg32 = Pcg32::from_parts(generator.state(), generator.increment()).unwrap();

        assert_eq!(restored.next_u32(), generator.next_u32());
        assert_eq!(restored, generator);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared pcg32`
Expected: the `shared` library tests report `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/src/lib.rs ./shared/src/random/mod.rs ./shared/src/random/pcg32.rs
git commit -m "Pcg32 random number generator"
```

`git status` before the commit lists only the paths staged above.

### Task 4: CellOccupancy

**Files:**
- Create: `shared/src/organism/mod.rs`
- Create: `shared/src/organism/cell_occupancy.rs`
- Modify: `shared/src/lib.rs`
- Test: inline `mod tests` in `shared/src/organism/cell_occupancy.rs`

- [ ] **Step 1: Write the failing test**

`shared/src/lib.rs`, whole file:

```rust
//! Everything the server and the clients must agree on, plus platform-independent client logic.

pub mod geometry;
pub mod organism;
pub mod random;
```

`shared/src/organism/mod.rs`:

```rust
pub mod cell_occupancy;

pub use cell_occupancy::*;
```

`shared/src/organism/cell_occupancy.rs`, tests only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn create_occupancy(lattice_coordinates: &[(i32, i32)]) -> CellOccupancy {
        let mut cell_occupancy: CellOccupancy = CellOccupancy::empty();

        for (i, j) in lattice_coordinates {
            cell_occupancy.insert(LatticeCoordinate { i: *i, j: *j });
        }

        cell_occupancy
    }

    #[test]
    fn insert_makes_cells_contained() {
        let cell_occupancy: CellOccupancy = create_occupancy(&[(0, 0), (-3, 2), (9, -1)]);

        assert!(cell_occupancy.contains(LatticeCoordinate { i: -3, j: 2 }));
        assert!(cell_occupancy.contains(LatticeCoordinate { i: 9, j: -1 }));
        assert!(!cell_occupancy.contains(LatticeCoordinate { i: 1, j: 0 }));
        assert!(!cell_occupancy.contains(LatticeCoordinate { i: 40, j: 40 }));
        assert_eq!(cell_occupancy.count(), 3);
    }

    #[test]
    fn remove_clears_only_that_cell() {
        let mut cell_occupancy: CellOccupancy = create_occupancy(&[(0, 0), (1, 0)]);
        cell_occupancy.remove(LatticeCoordinate { i: 0, j: 0 });
        cell_occupancy.remove(LatticeCoordinate { i: 50, j: 50 });

        assert!(!cell_occupancy.contains(LatticeCoordinate { i: 0, j: 0 }));
        assert!(cell_occupancy.contains(LatticeCoordinate { i: 1, j: 0 }));
        assert_eq!(cell_occupancy.count(), 1);
    }

    #[test]
    fn iter_yields_row_major_order() {
        let cell_occupancy: CellOccupancy = create_occupancy(&[(3, 0), (0, 1), (-2, 0), (1, -1), (12, 0)]);
        let lattice_coordinates: Vec<LatticeCoordinate> = cell_occupancy.iter().collect();

        assert_eq!(
            lattice_coordinates,
            vec![
                LatticeCoordinate { i: 1, j: -1 },
                LatticeCoordinate { i: -2, j: 0 },
                LatticeCoordinate { i: 3, j: 0 },
                LatticeCoordinate { i: 12, j: 0 },
                LatticeCoordinate { i: 0, j: 1 },
            ],
        );
    }

    #[test]
    fn retighten_shrinks_the_box_to_the_remaining_cells() {
        let mut cell_occupancy: CellOccupancy = create_occupancy(&[(0, 0), (20, 5), (1, 0)]);
        cell_occupancy.remove(LatticeCoordinate { i: 20, j: 5 });
        cell_occupancy.retighten();

        assert_eq!(cell_occupancy.origin(), LatticeCoordinate { i: 0, j: 0 });
        assert_eq!(cell_occupancy.width(), 2);
        assert_eq!(cell_occupancy.height(), 1);
        assert_eq!(cell_occupancy.row_bitmap_bytes(), &[0b0000_0011]);
    }

    #[test]
    fn retighten_gives_equal_values_for_equal_sets() {
        let mut first: CellOccupancy = create_occupancy(&[(0, 0), (1, 0), (0, 1)]);
        let mut second: CellOccupancy = create_occupancy(&[(0, 1), (-7, 9), (1, 0), (0, 0)]);
        second.remove(LatticeCoordinate { i: -7, j: 9 });

        first.retighten();
        second.retighten();

        assert_eq!(first, second);
    }

    #[test]
    fn retighten_of_no_cells_gives_empty() {
        let mut cell_occupancy: CellOccupancy = create_occupancy(&[(4, 4)]);
        cell_occupancy.remove(LatticeCoordinate { i: 4, j: 4 });
        cell_occupancy.retighten();

        assert_eq!(cell_occupancy, CellOccupancy::empty());
        assert!(cell_occupancy.is_empty());
    }

    #[test]
    fn from_tight_bitmap_accepts_its_own_parts() {
        let mut cell_occupancy: CellOccupancy = create_occupancy(&[(0, 0), (9, 1), (4, 2)]);
        cell_occupancy.retighten();

        let restored: Option<CellOccupancy> = CellOccupancy::from_tight_bitmap(
            cell_occupancy.origin(),
            cell_occupancy.width(),
            cell_occupancy.height(),
            cell_occupancy.row_bitmap_bytes().to_vec(),
        );

        assert_eq!(restored, Some(cell_occupancy));
    }

    #[test]
    fn from_tight_bitmap_rejects_wrong_length() {
        let origin: LatticeCoordinate = LatticeCoordinate { i: 0, j: 0 };

        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 9, 1, vec![0b1]), None);
        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 1, 1, vec![1, 0]), None);
    }

    #[test]
    fn from_tight_bitmap_rejects_non_tight_box() {
        let origin: LatticeCoordinate = LatticeCoordinate { i: 0, j: 0 };

        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 2, 1, vec![0b01]), None);
        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 1, 2, vec![0, 1]), None);
        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 0, 0, Vec::new()), None);
    }

    #[test]
    fn from_tight_bitmap_rejects_bits_past_width() {
        let origin: LatticeCoordinate = LatticeCoordinate { i: 0, j: 0 };

        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 2, 1, vec![0b0000_0111]), None);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared cell_occupancy`
Expected: FAIL, compilation stops; the first error is `` error[E0425]: cannot find type `CellOccupancy` in this scope ``.

- [ ] **Step 3: Write the implementation**

Replace the whole of `shared/src/organism/cell_occupancy.rs` with:

```rust
use crate::geometry::LatticeCoordinate;

/// Equal cell sets are equal values once both are re-tightened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellOccupancy {
    origin: LatticeCoordinate,
    width: u16,
    height: u16,
    /// Row-major, `ceil(width / 8)` bytes per row; column `c` is bit `c % 8` of byte `c / 8`.
    row_bitmap_bytes: Vec<u8>,
}

impl CellOccupancy {
    pub fn empty() -> CellOccupancy {
        CellOccupancy {
            origin: LatticeCoordinate { i: 0, j: 0 },
            width: 0,
            height: 0,
            row_bitmap_bytes: Vec::new(),
        }
    }

    pub fn with_cell(lattice_coordinate: LatticeCoordinate) -> CellOccupancy {
        CellOccupancy {
            origin: lattice_coordinate,
            width: 1,
            height: 1,
            row_bitmap_bytes: vec![1],
        }
    }

    /// `None` unless the box is non-empty, every edge row and column holds a cell, and no bit past `width` is set.
    pub fn from_tight_bitmap(
        origin: LatticeCoordinate,
        width: u16,
        height: u16,
        row_bitmap_bytes: Vec<u8>,
    ) -> Option<CellOccupancy> {
        if width == 0 || height == 0 {
            return None;
        }

        let expected_byte_count: usize = get_bytes_per_row(width) * usize::from(height);
        if row_bitmap_bytes.len() != expected_byte_count {
            return None;
        }

        let candidate: CellOccupancy = CellOccupancy {
            origin,
            width,
            height,
            row_bitmap_bytes,
        };

        let has_padding_bits: bool = candidate.has_bits_past_width();
        let tightened_box: Option<CellBox> = candidate.get_tight_box();
        if has_padding_bits || tightened_box != Some(candidate.get_box()) {
            return None;
        }

        Some(candidate)
    }

    pub fn origin(&self) -> LatticeCoordinate {
        self.origin
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    pub fn row_bitmap_bytes(&self) -> &[u8] {
        &self.row_bitmap_bytes
    }

    pub fn contains(&self, lattice_coordinate: LatticeCoordinate) -> bool {
        let Some(bit_position) = self.get_bit_position(lattice_coordinate) else {
            return false;
        };

        self.row_bitmap_bytes[bit_position.byte_index] & bit_position.mask != 0
    }

    pub fn insert(&mut self, lattice_coordinate: LatticeCoordinate) {
        if self.get_bit_position(lattice_coordinate).is_none() {
            self.expand_to_include(lattice_coordinate);
        }

        let bit_position: BitPosition = self.get_bit_position(lattice_coordinate).unwrap();
        self.row_bitmap_bytes[bit_position.byte_index] |= bit_position.mask;
    }

    pub fn remove(&mut self, lattice_coordinate: LatticeCoordinate) {
        let Some(bit_position) = self.get_bit_position(lattice_coordinate) else {
            return;
        };

        self.row_bitmap_bytes[bit_position.byte_index] &= !bit_position.mask;
    }

    pub fn count(&self) -> u32 {
        self.row_bitmap_bytes.iter().map(|byte| byte.count_ones()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.row_bitmap_bytes.iter().all(|byte| *byte == 0)
    }

    /// Ascending `LatticeCoordinate` order.
    pub fn iter(&self) -> impl Iterator<Item = LatticeCoordinate> + '_ {
        let rows: std::ops::Range<i32> = 0..i32::from(self.height);

        rows.flat_map(move |row| {
            let columns: std::ops::Range<i32> = 0..i32::from(self.width);

            columns.map(move |column| LatticeCoordinate {
                i: self.origin.i + column,
                j: self.origin.j + row,
            })
        })
        .filter(|lattice_coordinate| self.contains(*lattice_coordinate))
    }

    pub fn retighten(&mut self) {
        let Some(tight_box) = self.get_tight_box() else {
            *self = CellOccupancy::empty();
            return;
        };

        if tight_box != self.get_box() {
            self.rebuild(tight_box);
        }
    }

    fn get_box(&self) -> CellBox {
        CellBox {
            minimum: self.origin,
            maximum: LatticeCoordinate {
                i: self.origin.i + i32::from(self.width) - 1,
                j: self.origin.j + i32::from(self.height) - 1,
            },
        }
    }

    fn get_tight_box(&self) -> Option<CellBox> {
        let lattice_coordinates: Vec<LatticeCoordinate> = self.iter().collect();
        let first: LatticeCoordinate = *lattice_coordinates.first()?;
        let mut tight_box: CellBox = CellBox {
            minimum: first,
            maximum: first,
        };

        for lattice_coordinate in &lattice_coordinates[1..] {
            tight_box = tight_box.including(*lattice_coordinate);
        }

        Some(tight_box)
    }

    fn expand_to_include(&mut self, lattice_coordinate: LatticeCoordinate) {
        let expanded_box: CellBox = match self.get_tight_box() {
            Some(current_box) => current_box.including(lattice_coordinate),
            None => CellBox {
                minimum: lattice_coordinate,
                maximum: lattice_coordinate,
            },
        };

        self.rebuild(expanded_box);
    }

    fn rebuild(&mut self, cell_box: CellBox) {
        let lattice_coordinates: Vec<LatticeCoordinate> = self.iter().collect();
        let width: u16 = u16::try_from(cell_box.maximum.i - cell_box.minimum.i + 1).unwrap();
        let height: u16 = u16::try_from(cell_box.maximum.j - cell_box.minimum.j + 1).unwrap();

        *self = CellOccupancy {
            origin: cell_box.minimum,
            width,
            height,
            row_bitmap_bytes: vec![0; get_bytes_per_row(width) * usize::from(height)],
        };

        for lattice_coordinate in lattice_coordinates {
            self.insert(lattice_coordinate);
        }
    }

    fn get_bit_position(&self, lattice_coordinate: LatticeCoordinate) -> Option<BitPosition> {
        let column: i32 = lattice_coordinate.i - self.origin.i;
        let row: i32 = lattice_coordinate.j - self.origin.j;
        let is_inside_box: bool =
            (0..i32::from(self.width)).contains(&column) && (0..i32::from(self.height)).contains(&row);
        if !is_inside_box {
            return None;
        }

        let column_index: usize = usize::try_from(column).unwrap();
        let row_index: usize = usize::try_from(row).unwrap();

        Some(BitPosition {
            byte_index: row_index * get_bytes_per_row(self.width) + column_index / 8,
            mask: 1 << (column_index % 8),
        })
    }

    fn has_bits_past_width(&self) -> bool {
        let used_bits_in_last_byte: u16 = self.width % 8;
        if used_bits_in_last_byte == 0 {
            return false;
        }

        let padding_mask: u8 = !((1_u8 << used_bits_in_last_byte) - 1);
        let bytes_per_row: usize = get_bytes_per_row(self.width);

        self.row_bitmap_bytes
            .chunks(bytes_per_row)
            .any(|row_bytes| row_bytes[bytes_per_row - 1] & padding_mask != 0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CellBox {
    minimum: LatticeCoordinate,
    maximum: LatticeCoordinate,
}

impl CellBox {
    fn including(self, lattice_coordinate: LatticeCoordinate) -> CellBox {
        CellBox {
            minimum: LatticeCoordinate {
                i: self.minimum.i.min(lattice_coordinate.i),
                j: self.minimum.j.min(lattice_coordinate.j),
            },
            maximum: LatticeCoordinate {
                i: self.maximum.i.max(lattice_coordinate.i),
                j: self.maximum.j.max(lattice_coordinate.j),
            },
        }
    }
}

struct BitPosition {
    byte_index: usize,
    mask: u8,
}

fn get_bytes_per_row(width: u16) -> usize {
    usize::from(width).div_ceil(8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_occupancy(lattice_coordinates: &[(i32, i32)]) -> CellOccupancy {
        let mut cell_occupancy: CellOccupancy = CellOccupancy::empty();

        for (i, j) in lattice_coordinates {
            cell_occupancy.insert(LatticeCoordinate { i: *i, j: *j });
        }

        cell_occupancy
    }

    #[test]
    fn insert_makes_cells_contained() {
        let cell_occupancy: CellOccupancy = create_occupancy(&[(0, 0), (-3, 2), (9, -1)]);

        assert!(cell_occupancy.contains(LatticeCoordinate { i: -3, j: 2 }));
        assert!(cell_occupancy.contains(LatticeCoordinate { i: 9, j: -1 }));
        assert!(!cell_occupancy.contains(LatticeCoordinate { i: 1, j: 0 }));
        assert!(!cell_occupancy.contains(LatticeCoordinate { i: 40, j: 40 }));
        assert_eq!(cell_occupancy.count(), 3);
    }

    #[test]
    fn remove_clears_only_that_cell() {
        let mut cell_occupancy: CellOccupancy = create_occupancy(&[(0, 0), (1, 0)]);
        cell_occupancy.remove(LatticeCoordinate { i: 0, j: 0 });
        cell_occupancy.remove(LatticeCoordinate { i: 50, j: 50 });

        assert!(!cell_occupancy.contains(LatticeCoordinate { i: 0, j: 0 }));
        assert!(cell_occupancy.contains(LatticeCoordinate { i: 1, j: 0 }));
        assert_eq!(cell_occupancy.count(), 1);
    }

    #[test]
    fn iter_yields_row_major_order() {
        let cell_occupancy: CellOccupancy = create_occupancy(&[(3, 0), (0, 1), (-2, 0), (1, -1), (12, 0)]);
        let lattice_coordinates: Vec<LatticeCoordinate> = cell_occupancy.iter().collect();

        assert_eq!(
            lattice_coordinates,
            vec![
                LatticeCoordinate { i: 1, j: -1 },
                LatticeCoordinate { i: -2, j: 0 },
                LatticeCoordinate { i: 3, j: 0 },
                LatticeCoordinate { i: 12, j: 0 },
                LatticeCoordinate { i: 0, j: 1 },
            ],
        );
    }

    #[test]
    fn retighten_shrinks_the_box_to_the_remaining_cells() {
        let mut cell_occupancy: CellOccupancy = create_occupancy(&[(0, 0), (20, 5), (1, 0)]);
        cell_occupancy.remove(LatticeCoordinate { i: 20, j: 5 });
        cell_occupancy.retighten();

        assert_eq!(cell_occupancy.origin(), LatticeCoordinate { i: 0, j: 0 });
        assert_eq!(cell_occupancy.width(), 2);
        assert_eq!(cell_occupancy.height(), 1);
        assert_eq!(cell_occupancy.row_bitmap_bytes(), &[0b0000_0011]);
    }

    #[test]
    fn retighten_gives_equal_values_for_equal_sets() {
        let mut first: CellOccupancy = create_occupancy(&[(0, 0), (1, 0), (0, 1)]);
        let mut second: CellOccupancy = create_occupancy(&[(0, 1), (-7, 9), (1, 0), (0, 0)]);
        second.remove(LatticeCoordinate { i: -7, j: 9 });

        first.retighten();
        second.retighten();

        assert_eq!(first, second);
    }

    #[test]
    fn retighten_of_no_cells_gives_empty() {
        let mut cell_occupancy: CellOccupancy = create_occupancy(&[(4, 4)]);
        cell_occupancy.remove(LatticeCoordinate { i: 4, j: 4 });
        cell_occupancy.retighten();

        assert_eq!(cell_occupancy, CellOccupancy::empty());
        assert!(cell_occupancy.is_empty());
    }

    #[test]
    fn from_tight_bitmap_accepts_its_own_parts() {
        let mut cell_occupancy: CellOccupancy = create_occupancy(&[(0, 0), (9, 1), (4, 2)]);
        cell_occupancy.retighten();

        let restored: Option<CellOccupancy> = CellOccupancy::from_tight_bitmap(
            cell_occupancy.origin(),
            cell_occupancy.width(),
            cell_occupancy.height(),
            cell_occupancy.row_bitmap_bytes().to_vec(),
        );

        assert_eq!(restored, Some(cell_occupancy));
    }

    #[test]
    fn from_tight_bitmap_rejects_wrong_length() {
        let origin: LatticeCoordinate = LatticeCoordinate { i: 0, j: 0 };

        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 9, 1, vec![0b1]), None);
        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 1, 1, vec![1, 0]), None);
    }

    #[test]
    fn from_tight_bitmap_rejects_non_tight_box() {
        let origin: LatticeCoordinate = LatticeCoordinate { i: 0, j: 0 };

        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 2, 1, vec![0b01]), None);
        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 1, 2, vec![0, 1]), None);
        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 0, 0, Vec::new()), None);
    }

    #[test]
    fn from_tight_bitmap_rejects_bits_past_width() {
        let origin: LatticeCoordinate = LatticeCoordinate { i: 0, j: 0 };

        assert_eq!(CellOccupancy::from_tight_bitmap(origin, 2, 1, vec![0b0000_0111]), None);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared cell_occupancy`
Expected: the `shared` library tests report `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/src/lib.rs ./shared/src/organism/cell_occupancy.rs ./shared/src/organism/mod.rs
git commit -m "cell occupancy bitmap"
```

`git status` before the commit lists only the paths staged above.

### Task 5: Member kinds and team colours

**Files:**
- Create: `shared/src/member/mod.rs`
- Create: `shared/src/member/member.rs`
- Modify: `shared/src/lib.rs`
- Test: inline `mod tests` in `shared/src/member/member.rs`

`Member` itself arrives in Task 11, once `Organism` and `Score` exist.

- [ ] **Step 1: Write the failing test**

`shared/src/lib.rs`, whole file:

```rust
//! Everything the server and the clients must agree on, plus platform-independent client logic.

pub mod geometry;
pub mod member;
pub mod organism;
pub mod random;
```

`shared/src/member/mod.rs`:

```rust
pub mod member;

pub use member::*;
```

`shared/src/member/member.rs`, tests only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_team_color_forces_the_team_color_and_keeps_the_skin() {
        let appearance: Appearance = Appearance {
            color: OrganismColorKind::Camel,
            skin: SkinKind::Ghost,
        };

        assert_eq!(
            appearance.with_team_color(Some(TeamKind::Blue)),
            Appearance {
                color: OrganismColorKind::Sky,
                skin: SkinKind::Ghost,
            },
        );
        assert_eq!(appearance.with_team_color(None), appearance);
    }

    #[test]
    fn forced_color_maps_each_team() {
        let forced_colors: Vec<OrganismColorKind> = [TeamKind::Red, TeamKind::Blue, TeamKind::Green, TeamKind::Pink]
            .iter()
            .map(|team| team.forced_color())
            .collect();

        assert_eq!(
            forced_colors,
            vec![
                OrganismColorKind::Fire,
                OrganismColorKind::Sky,
                OrganismColorKind::Lime,
                OrganismColorKind::Petal,
            ],
        );
    }

    #[test]
    fn next_saturates_at_the_maximum_id() {
        assert_eq!(MemberId(4).next(), MemberId(5));
        assert_eq!(MemberId(u32::MAX).next(), MemberId(u32::MAX));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared member::member`
Expected: FAIL, compilation stops; the first error is `` error[E0425]: cannot find type `Appearance` in this scope ``.

- [ ] **Step 3: Write the implementation**

Replace the whole of `shared/src/member/member.rs` with:

```rust
/// Never reused within a game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemberId(pub u32);

impl MemberId {
    pub fn next(self) -> MemberId {
        MemberId(self.0.saturating_add(1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemberRoleKind {
    Participant,
    Spectator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Appearance {
    pub color: OrganismColorKind,
    pub skin: SkinKind,
}

impl Appearance {
    /// A team member's colour is always its team's colour.
    pub fn with_team_color(self, team: Option<TeamKind>) -> Appearance {
        let Some(team) = team else {
            return self;
        };

        Appearance {
            color: team.forced_color(),
            skin: self.skin,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrganismColorKind {
    Fire,
    Camel,
    Clay,
    Sun,
    Leaf,
    Lime,
    Sky,
    Lake,
    Ocean,
    Royal,
    Petal,
    Hot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkinKind {
    Grid,
    Circles,
    Ghost,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TeamKind {
    Red,
    Blue,
    Green,
    Pink,
}

impl TeamKind {
    pub fn forced_color(self) -> OrganismColorKind {
        match self {
            TeamKind::Red => OrganismColorKind::Fire,
            TeamKind::Blue => OrganismColorKind::Sky,
            TeamKind::Green => OrganismColorKind::Lime,
            TeamKind::Pink => OrganismColorKind::Petal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_team_color_forces_the_team_color_and_keeps_the_skin() {
        let appearance: Appearance = Appearance {
            color: OrganismColorKind::Camel,
            skin: SkinKind::Ghost,
        };

        assert_eq!(
            appearance.with_team_color(Some(TeamKind::Blue)),
            Appearance {
                color: OrganismColorKind::Sky,
                skin: SkinKind::Ghost,
            },
        );
        assert_eq!(appearance.with_team_color(None), appearance);
    }

    #[test]
    fn forced_color_maps_each_team() {
        let forced_colors: Vec<OrganismColorKind> = [TeamKind::Red, TeamKind::Blue, TeamKind::Green, TeamKind::Pink]
            .iter()
            .map(|team| team.forced_color())
            .collect();

        assert_eq!(
            forced_colors,
            vec![
                OrganismColorKind::Fire,
                OrganismColorKind::Sky,
                OrganismColorKind::Lime,
                OrganismColorKind::Petal,
            ],
        );
    }

    #[test]
    fn next_saturates_at_the_maximum_id() {
        assert_eq!(MemberId(4).next(), MemberId(5));
        assert_eq!(MemberId(u32::MAX).next(), MemberId(u32::MAX));
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared member::member`
Expected: the `shared` library tests report `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 21 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/src/lib.rs ./shared/src/member/member.rs ./shared/src/member/mod.rs
git commit -m "member kinds and team colours"
```

`git status` before the commit lists only the paths staged above.

### Task 6: Tick and the ability model

**Files:**
- Create: `shared/src/game/mod.rs`
- Create: `shared/src/game/tick.rs`
- Create: `shared/src/ability/mod.rs`
- Create: `shared/src/ability/ability_model.rs`
- Modify: `shared/src/lib.rs`
- Test: inline `mod tests` in `shared/src/ability/ability_model.rs`

Data and predicates only; activation, projectiles and damage are phase 3. The predicates are the only way growth reads ability state, since `first`, `second` and `third` are shared between a self-cast and a caster-side timer.

- [ ] **Step 1: Write the failing test**

`shared/src/lib.rs`, whole file:

```rust
//! Everything the server and the clients must agree on, plus platform-independent client logic.

pub mod ability;
pub mod game;
pub mod geometry;
pub mod member;
pub mod organism;
pub mod random;
```

`shared/src/game/mod.rs`:

```rust
pub mod tick;

pub use tick::*;
```

`shared/src/game/tick.rs` (a plain newtype; nothing to test on its own):

```rust
pub const TICK_PERIOD_MILLISECONDS: u32 = 70;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick(pub u32);

impl Tick {
    pub fn next(self) -> Tick {
        Tick(self.0.wrapping_add(1))
    }
}
```

`shared/src/ability/mod.rs`:

```rust
pub mod ability_model;

pub use ability_model::*;
```

`shared/src/ability/ability_model.rs`, tests only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::member::{OrganismColorKind, SkinKind};

    fn create_loadout(first: FirstAbilityKind, second: SecondAbilityKind, third: ThirdAbilityKind) -> Loadout {
        Loadout {
            appearance: Appearance {
                color: OrganismColorKind::Leaf,
                skin: SkinKind::Grid,
            },
            first,
            second,
            third,
        }
    }

    fn create_active_abilities() -> OrganismAbilities {
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.first = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.second = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third = AbilityPhase::Active { ends_at: Tick(50) };

        abilities
    }

    #[test]
    fn is_extended_requires_the_extend_loadout() {
        let abilities: OrganismAbilities = create_active_abilities();
        let extend_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Neutralize,
        );
        let compress_loadout: Loadout = create_loadout(
            FirstAbilityKind::Compress,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Neutralize,
        );

        assert!(abilities.is_extended(&extend_loadout));
        assert!(!abilities.is_extended(&compress_loadout));
        assert!(!OrganismAbilities::all_ready().is_extended(&extend_loadout));
    }

    #[test]
    fn is_immortal_requires_the_immortality_loadout() {
        let abilities: OrganismAbilities = create_active_abilities();
        let immortality_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Neutralize,
        );
        let freeze_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Freeze,
            ThirdAbilityKind::Neutralize,
        );

        assert!(abilities.is_immortal(&immortality_loadout));
        assert!(!abilities.is_immortal(&freeze_loadout));
    }

    #[test]
    fn is_neutralize_field_active_and_is_toxin_field_active_follow_the_third_ability_kind() {
        let abilities: OrganismAbilities = create_active_abilities();
        let neutralize_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Neutralize,
        );
        let toxin_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Toxin,
        );

        assert!(abilities.is_neutralize_field_active(&neutralize_loadout));
        assert!(!abilities.is_toxin_field_active(&neutralize_loadout));
        assert!(abilities.is_toxin_field_active(&toxin_loadout));
        assert!(!abilities.is_neutralize_field_active(&toxin_loadout));
    }

    #[test]
    fn is_compressed_and_is_frozen_hold_while_a_deadline_is_set() {
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();

        assert!(!abilities.is_compressed());
        assert!(!abilities.is_frozen());

        abilities.compressed_until = Some(Tick(10));
        abilities.frozen_until = Some(Tick(10));

        assert!(abilities.is_compressed());
        assert!(abilities.is_frozen());
    }

    #[test]
    fn from_bits_rejects_unknown_bits() {
        assert_eq!(
            AbilityPressSet::from_bits(0b1111).map(AbilityPressSet::bits),
            Some(0b1111),
        );
        assert_eq!(AbilityPressSet::from_bits(0b1_0000), None);
        assert_eq!(AbilityPressSet::from_bits(0), Some(AbilityPressSet::NONE));
    }

    #[test]
    fn with_team_color_forces_only_the_color() {
        let loadout: Loadout = create_loadout(
            FirstAbilityKind::Compress,
            SecondAbilityKind::Freeze,
            ThirdAbilityKind::Toxin,
        );
        let team_loadout: Loadout = loadout.with_team_color(Some(TeamKind::Pink));

        assert_eq!(team_loadout.appearance.color, OrganismColorKind::Petal);
        assert_eq!(team_loadout.appearance.skin, SkinKind::Grid);
        assert_eq!(team_loadout.first, FirstAbilityKind::Compress);
        assert_eq!(team_loadout.second, SecondAbilityKind::Freeze);
        assert_eq!(team_loadout.third, ThirdAbilityKind::Toxin);
        assert_eq!(loadout.with_team_color(None), loadout);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared ability_model`
Expected: FAIL, compilation stops; the first error is `` error[E0425]: cannot find type `FirstAbilityKind` in this scope ``.

- [ ] **Step 3: Write the implementation**

Replace the whole of `shared/src/ability/ability_model.rs` with:

```rust
use crate::game::Tick;
use crate::geometry::{SubpixelPoint, SubpixelVector, WorldPoint};
use crate::member::{Appearance, TeamKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loadout {
    pub appearance: Appearance,
    pub first: FirstAbilityKind,
    pub second: SecondAbilityKind,
    pub third: ThirdAbilityKind,
}

impl Loadout {
    pub fn with_team_color(self, team: Option<TeamKind>) -> Loadout {
        Loadout {
            appearance: self.appearance.with_team_color(team),
            first: self.first,
            second: self.second,
            third: self.third,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstAbilityKind {
    Extend,
    Compress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondAbilityKind {
    Immortality,
    Freeze,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThirdAbilityKind {
    Neutralize,
    Toxin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbilityPhase {
    Ready,
    Active { ends_at: Tick },
    Cooling { ready_at: Tick },
}

impl AbilityPhase {
    pub fn is_active(self) -> bool {
        matches!(self, AbilityPhase::Active { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SporePhase {
    Ready,
    Flying { ends_at: Tick, spores: Vec<Projectile> },
    Secreting { ends_at: Tick, spores: Vec<Projectile> },
    Cooling { ready_at: Tick },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotPhase {
    Ready,
    Flying { ends_at: Tick, shot: Projectile },
    Secreting { ends_at: Tick, center: SubpixelPoint },
    Cooling { ready_at: Tick },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Projectile {
    pub position: SubpixelPoint,
    pub velocity: SubpixelVector,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrganismAbilities {
    pub first: AbilityPhase,
    pub second: AbilityPhase,
    pub third: AbilityPhase,
    pub third_center: Option<WorldPoint>,
    pub spore: SporePhase,
    /// Slot 0 carries compress, slot 1 carries freeze.
    pub shots: [ShotPhase; 2],
    pub compressed_until: Option<Tick>,
    pub frozen_until: Option<Tick>,
}

impl OrganismAbilities {
    pub fn all_ready() -> OrganismAbilities {
        OrganismAbilities {
            first: AbilityPhase::Ready,
            second: AbilityPhase::Ready,
            third: AbilityPhase::Ready,
            third_center: None,
            spore: SporePhase::Ready,
            shots: [ShotPhase::Ready, ShotPhase::Ready],
            compressed_until: None,
            frozen_until: None,
        }
    }

    pub fn is_extended(&self, loadout: &Loadout) -> bool {
        self.first.is_active() && loadout.first == FirstAbilityKind::Extend
    }

    pub fn is_immortal(&self, loadout: &Loadout) -> bool {
        self.second.is_active() && loadout.second == SecondAbilityKind::Immortality
    }

    pub fn is_neutralize_field_active(&self, loadout: &Loadout) -> bool {
        self.third.is_active() && loadout.third == ThirdAbilityKind::Neutralize
    }

    pub fn is_toxin_field_active(&self, loadout: &Loadout) -> bool {
        self.third.is_active() && loadout.third == ThirdAbilityKind::Toxin
    }

    pub fn is_compressed(&self) -> bool {
        self.compressed_until.is_some()
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen_until.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbilityPressSet {
    bits: u8,
}

impl AbilityPressSet {
    pub const NONE: AbilityPressSet = AbilityPressSet { bits: 0 };
    pub const FIRST: AbilityPressSet = AbilityPressSet { bits: 1 };
    pub const SECOND: AbilityPressSet = AbilityPressSet { bits: 1 << 1 };
    pub const THIRD: AbilityPressSet = AbilityPressSet { bits: 1 << 2 };
    pub const FOURTH: AbilityPressSet = AbilityPressSet { bits: 1 << 3 };
    const KNOWN_BITS: u8 = 0b1111;

    /// `None` when any bit other than the four press bits is set.
    pub fn from_bits(bits: u8) -> Option<AbilityPressSet> {
        if bits & !AbilityPressSet::KNOWN_BITS != 0 {
            return None;
        }

        Some(AbilityPressSet { bits })
    }

    pub fn bits(self) -> u8 {
        self.bits
    }
}

/// Mouse offset from the on-screen crosshair, in CSS px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AimVector {
    pub x: i16,
    pub y: i16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::member::{OrganismColorKind, SkinKind};

    fn create_loadout(first: FirstAbilityKind, second: SecondAbilityKind, third: ThirdAbilityKind) -> Loadout {
        Loadout {
            appearance: Appearance {
                color: OrganismColorKind::Leaf,
                skin: SkinKind::Grid,
            },
            first,
            second,
            third,
        }
    }

    fn create_active_abilities() -> OrganismAbilities {
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.first = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.second = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third = AbilityPhase::Active { ends_at: Tick(50) };

        abilities
    }

    #[test]
    fn is_extended_requires_the_extend_loadout() {
        let abilities: OrganismAbilities = create_active_abilities();
        let extend_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Neutralize,
        );
        let compress_loadout: Loadout = create_loadout(
            FirstAbilityKind::Compress,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Neutralize,
        );

        assert!(abilities.is_extended(&extend_loadout));
        assert!(!abilities.is_extended(&compress_loadout));
        assert!(!OrganismAbilities::all_ready().is_extended(&extend_loadout));
    }

    #[test]
    fn is_immortal_requires_the_immortality_loadout() {
        let abilities: OrganismAbilities = create_active_abilities();
        let immortality_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Neutralize,
        );
        let freeze_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Freeze,
            ThirdAbilityKind::Neutralize,
        );

        assert!(abilities.is_immortal(&immortality_loadout));
        assert!(!abilities.is_immortal(&freeze_loadout));
    }

    #[test]
    fn is_neutralize_field_active_and_is_toxin_field_active_follow_the_third_ability_kind() {
        let abilities: OrganismAbilities = create_active_abilities();
        let neutralize_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Neutralize,
        );
        let toxin_loadout: Loadout = create_loadout(
            FirstAbilityKind::Extend,
            SecondAbilityKind::Immortality,
            ThirdAbilityKind::Toxin,
        );

        assert!(abilities.is_neutralize_field_active(&neutralize_loadout));
        assert!(!abilities.is_toxin_field_active(&neutralize_loadout));
        assert!(abilities.is_toxin_field_active(&toxin_loadout));
        assert!(!abilities.is_neutralize_field_active(&toxin_loadout));
    }

    #[test]
    fn is_compressed_and_is_frozen_hold_while_a_deadline_is_set() {
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();

        assert!(!abilities.is_compressed());
        assert!(!abilities.is_frozen());

        abilities.compressed_until = Some(Tick(10));
        abilities.frozen_until = Some(Tick(10));

        assert!(abilities.is_compressed());
        assert!(abilities.is_frozen());
    }

    #[test]
    fn from_bits_rejects_unknown_bits() {
        assert_eq!(
            AbilityPressSet::from_bits(0b1111).map(AbilityPressSet::bits),
            Some(0b1111),
        );
        assert_eq!(AbilityPressSet::from_bits(0b1_0000), None);
        assert_eq!(AbilityPressSet::from_bits(0), Some(AbilityPressSet::NONE));
    }

    #[test]
    fn with_team_color_forces_only_the_color() {
        let loadout: Loadout = create_loadout(
            FirstAbilityKind::Compress,
            SecondAbilityKind::Freeze,
            ThirdAbilityKind::Toxin,
        );
        let team_loadout: Loadout = loadout.with_team_color(Some(TeamKind::Pink));

        assert_eq!(team_loadout.appearance.color, OrganismColorKind::Petal);
        assert_eq!(team_loadout.appearance.skin, SkinKind::Grid);
        assert_eq!(team_loadout.first, FirstAbilityKind::Compress);
        assert_eq!(team_loadout.second, SecondAbilityKind::Freeze);
        assert_eq!(team_loadout.third, ThirdAbilityKind::Toxin);
        assert_eq!(loadout.with_team_color(None), loadout);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared ability_model`
Expected: the `shared` library tests report `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 24 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/src/ability/ability_model.rs ./shared/src/ability/mod.rs ./shared/src/game/mod.rs ./shared/src/game/tick.rs ./shared/src/lib.rs
git commit -m "tick and ability model with predicates"
```

`git status` before the commit lists only the paths staged above.

### Task 7: Organism with exposed and adjacent enumeration

**Files:**
- Create: `shared/src/organism/organism.rs`
- Modify: `shared/src/organism/mod.rs`
- Test: inline `mod tests` in `shared/src/organism/organism.rs`

- [ ] **Step 1: Write the failing test**

`shared/src/organism/mod.rs`, whole file:

```rust
pub mod cell_occupancy;
pub mod organism;

pub use cell_occupancy::*;
pub use organism::*;
```

`shared/src/organism/organism.rs`, tests only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn create_organism(lattice_coordinates: &[(i32, i32)]) -> Organism {
        let mut organism: Organism = Organism::new(WorldPoint { x: 0, y: 0 });
        organism.cells = CellOccupancy::empty();

        for (i, j) in lattice_coordinates {
            organism.cells.insert(LatticeCoordinate { i: *i, j: *j });
        }

        organism
    }

    fn count_occurrences(lattice_coordinates: &[LatticeCoordinate], target: LatticeCoordinate) -> usize {
        lattice_coordinates.iter().filter(|lattice_coordinate| **lattice_coordinate == target).count()
    }

    #[test]
    fn new_places_one_cell_at_the_position() {
        let organism: Organism = Organism::new(WorldPoint { x: 40, y: 70 });

        assert_eq!(organism.anchor, WorldPoint { x: 40, y: 70 });
        assert_eq!(organism.cursor, WorldPoint { x: 40, y: 70 });
        assert_eq!(
            organism.cells.iter().collect::<Vec<LatticeCoordinate>>(),
            vec![LatticeCoordinate { i: 0, j: 0 }],
        );
        assert_eq!(
            organism.cell_center(LatticeCoordinate { i: 0, j: 0 }),
            WorldPoint { x: 40, y: 70 },
        );
        assert_eq!(organism.abilities, OrganismAbilities::all_ready());
        assert_eq!(organism.last_hitter, None);
    }

    #[test]
    fn exposed_cells_excludes_enclosed_cells() {
        let block: Vec<(i32, i32)> = (0..3).flat_map(|j| (0..3).map(move |i| (i, j))).collect();
        let organism: Organism = create_organism(&block);
        let exposed_cells: Vec<LatticeCoordinate> = organism.exposed_cells();

        assert_eq!(exposed_cells.len(), 8);
        assert!(!exposed_cells.contains(&LatticeCoordinate { i: 1, j: 1 }));
        assert_eq!(exposed_cells[0], LatticeCoordinate { i: 0, j: 0 });
        assert_eq!(exposed_cells[7], LatticeCoordinate { i: 2, j: 2 });
    }

    #[test]
    fn adjacent_sites_of_one_cell_follow_direction_order() {
        let organism: Organism = create_organism(&[(0, 0)]);

        assert_eq!(
            organism.adjacent_sites(),
            vec![
                LatticeCoordinate { i: -1, j: 0 },
                LatticeCoordinate { i: 0, j: -1 },
                LatticeCoordinate { i: 1, j: 0 },
                LatticeCoordinate { i: 0, j: 1 },
            ],
        );
    }

    #[test]
    fn adjacent_sites_repeat_a_site_once_per_neighboring_cell() {
        let organism: Organism = create_organism(&[(0, 0), (2, 0), (1, 1)]);
        let adjacent_sites: Vec<LatticeCoordinate> = organism.adjacent_sites();

        assert_eq!(adjacent_sites.len(), 12);
        assert_eq!(count_occurrences(&adjacent_sites, LatticeCoordinate { i: 1, j: 0 }), 3);
        assert_eq!(count_occurrences(&adjacent_sites, LatticeCoordinate { i: 0, j: 1 }), 2);
        assert_eq!(count_occurrences(&adjacent_sites, LatticeCoordinate { i: -1, j: 0 }), 1);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared organism::organism`
Expected: FAIL, compilation stops; the first error is `` error[E0425]: cannot find type `Organism` in this scope ``.

- [ ] **Step 3: Write the implementation**

Replace the whole of `shared/src/organism/organism.rs` with:

```rust
use crate::ability::OrganismAbilities;
use crate::geometry;
use crate::geometry::{LatticeCoordinate, WorldPoint};
use crate::member::MemberId;
use crate::organism::CellOccupancy;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Organism {
    /// World position of lattice coordinate (0, 0).
    pub anchor: WorldPoint,
    pub cells: CellOccupancy,
    pub cursor: WorldPoint,
    pub last_hitter: Option<MemberId>,
    pub abilities: OrganismAbilities,
}

impl Organism {
    /// One cell at `position`, with the anchor and the cursor there.
    pub fn new(position: WorldPoint) -> Organism {
        Organism {
            anchor: position,
            cells: CellOccupancy::with_cell(LatticeCoordinate { i: 0, j: 0 }),
            cursor: position,
            last_hitter: None,
            abilities: OrganismAbilities::all_ready(),
        }
    }

    pub fn cell_center(&self, lattice_coordinate: LatticeCoordinate) -> WorldPoint {
        lattice_coordinate.to_cell_center(self.anchor)
    }

    /// Cells with fewer than four orthogonal own neighbours, in lattice order.
    pub fn exposed_cells(&self) -> Vec<LatticeCoordinate> {
        self.cells
            .iter()
            .filter(|lattice_coordinate| {
                geometry::NEIGHBOR_DIRECTIONS
                    .iter()
                    .any(|direction| !self.cells.contains(lattice_coordinate.neighbor(*direction)))
            })
            .collect()
    }

    /// For each cell in lattice order, each empty neighbour in direction order; a site next to k cells appears k
    /// times.
    pub fn adjacent_sites(&self) -> Vec<LatticeCoordinate> {
        let mut adjacent_sites: Vec<LatticeCoordinate> = Vec::new();

        for lattice_coordinate in self.cells.iter() {
            for direction in geometry::NEIGHBOR_DIRECTIONS {
                let neighbor: LatticeCoordinate = lattice_coordinate.neighbor(direction);
                if !self.cells.contains(neighbor) {
                    adjacent_sites.push(neighbor);
                }
            }
        }

        adjacent_sites
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_organism(lattice_coordinates: &[(i32, i32)]) -> Organism {
        let mut organism: Organism = Organism::new(WorldPoint { x: 0, y: 0 });
        organism.cells = CellOccupancy::empty();

        for (i, j) in lattice_coordinates {
            organism.cells.insert(LatticeCoordinate { i: *i, j: *j });
        }

        organism
    }

    fn count_occurrences(lattice_coordinates: &[LatticeCoordinate], target: LatticeCoordinate) -> usize {
        lattice_coordinates.iter().filter(|lattice_coordinate| **lattice_coordinate == target).count()
    }

    #[test]
    fn new_places_one_cell_at_the_position() {
        let organism: Organism = Organism::new(WorldPoint { x: 40, y: 70 });

        assert_eq!(organism.anchor, WorldPoint { x: 40, y: 70 });
        assert_eq!(organism.cursor, WorldPoint { x: 40, y: 70 });
        assert_eq!(
            organism.cells.iter().collect::<Vec<LatticeCoordinate>>(),
            vec![LatticeCoordinate { i: 0, j: 0 }],
        );
        assert_eq!(
            organism.cell_center(LatticeCoordinate { i: 0, j: 0 }),
            WorldPoint { x: 40, y: 70 },
        );
        assert_eq!(organism.abilities, OrganismAbilities::all_ready());
        assert_eq!(organism.last_hitter, None);
    }

    #[test]
    fn exposed_cells_excludes_enclosed_cells() {
        let block: Vec<(i32, i32)> = (0..3).flat_map(|j| (0..3).map(move |i| (i, j))).collect();
        let organism: Organism = create_organism(&block);
        let exposed_cells: Vec<LatticeCoordinate> = organism.exposed_cells();

        assert_eq!(exposed_cells.len(), 8);
        assert!(!exposed_cells.contains(&LatticeCoordinate { i: 1, j: 1 }));
        assert_eq!(exposed_cells[0], LatticeCoordinate { i: 0, j: 0 });
        assert_eq!(exposed_cells[7], LatticeCoordinate { i: 2, j: 2 });
    }

    #[test]
    fn adjacent_sites_of_one_cell_follow_direction_order() {
        let organism: Organism = create_organism(&[(0, 0)]);

        assert_eq!(
            organism.adjacent_sites(),
            vec![
                LatticeCoordinate { i: -1, j: 0 },
                LatticeCoordinate { i: 0, j: -1 },
                LatticeCoordinate { i: 1, j: 0 },
                LatticeCoordinate { i: 0, j: 1 },
            ],
        );
    }

    #[test]
    fn adjacent_sites_repeat_a_site_once_per_neighboring_cell() {
        let organism: Organism = create_organism(&[(0, 0), (2, 0), (1, 1)]);
        let adjacent_sites: Vec<LatticeCoordinate> = organism.adjacent_sites();

        assert_eq!(adjacent_sites.len(), 12);
        assert_eq!(count_occurrences(&adjacent_sites, LatticeCoordinate { i: 1, j: 0 }), 3);
        assert_eq!(count_occurrences(&adjacent_sites, LatticeCoordinate { i: 0, j: 1 }), 2);
        assert_eq!(count_occurrences(&adjacent_sites, LatticeCoordinate { i: -1, j: 0 }), 1);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared organism::organism`
Expected: the `shared` library tests report `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 30 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/src/organism/mod.rs ./shared/src/organism/organism.rs
git commit -m "organism with exposed and adjacent enumeration"
```

`git status` before the commit lists only the paths staged above.

### Task 8: World bounds and the border rule

**Files:**
- Create: `shared/src/world/mod.rs`
- Create: `shared/src/world/world.rs`
- Modify: `shared/src/lib.rs`
- Test: inline `mod tests` in `shared/src/world/world.rs`

Worked values used by the tests: an 800 px circle has centre (400, 400) and radius 400; the cell at (400, 6) has the corner (394, 0), and `6² + 400² >= 400²` puts it outside, while (400, 7) has the corner (394, 1) with `6² + 399² = 159237 < 160000`. After 1000 shrinks `left = 143_000` subpixels; a cell at x = 145 has `145 * 1024 - 6144 = 142336 <= 143000` (outside) and x = 146 has `143360` (inside). The 800 px ellipse after 1000 shrinks has `left = top = 143_000` and `width = height = 533_200` subpixels about the unchanged centre `409_600`; the cell at (400, 145) has a top corner with `2dy = 2 * 142336 - 819200 = -534528`, past the semi-axis (outside), while (400, 146) gives `12288² + 532480² = 283_685_945_344 < 533200² = 284_302_240_000` (inside).

- [ ] **Step 1: Write the failing test**

`shared/src/lib.rs`, whole file:

```rust
//! Everything the server and the clients must agree on, plus platform-independent client logic.

pub mod ability;
pub mod game;
pub mod geometry;
pub mod member;
pub mod organism;
pub mod random;
pub mod world;
```

`shared/src/world/mod.rs`:

```rust
pub mod world;

pub use world::*;
```

`shared/src/world/world.rs`, tests only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn create_world(shape: WorldShapeKind, left_pixels: i32, width_pixels: u32, height_pixels: u32) -> World {
        let mut bounds: WorldBounds = WorldBounds::from_pixel_size(width_pixels, height_pixels);
        bounds.left = Subpixels(left_pixels * geometry::SUBPIXELS_PER_PIXEL);

        World::new(shape, bounds)
    }

    #[test]
    fn contains_cell_rectangle_excludes_touching_the_border() {
        let world: World = create_world(WorldShapeKind::Rectangle, 0, 800, 800);

        assert!(!world.contains_cell(WorldPoint { x: 6, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 7, y: 400 }));
        assert!(!world.contains_cell(WorldPoint { x: 794, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 793, y: 400 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 6 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 794 }));
    }

    #[test]
    fn contains_cell_rectangle_follows_an_offset_origin() {
        let world: World = create_world(WorldShapeKind::Rectangle, 1000, 800, 800);

        assert!(!world.contains_cell(WorldPoint { x: 1006, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 1007, y: 400 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 400 }));
    }

    #[test]
    fn contains_cell_rectangle_follows_shrunk_subpixel_bounds() {
        let mut world: World = create_world(WorldShapeKind::Rectangle, 0, 800, 800);
        for _ in 0..1000 {
            world.shrink();
        }

        assert_eq!(world.bounds.left, Subpixels(143_000));
        assert!(!world.contains_cell(WorldPoint { x: 145, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 146, y: 400 }));
    }

    #[test]
    fn contains_cell_ellipse_tests_every_corner() {
        let world: World = create_world(WorldShapeKind::Ellipse, 0, 800, 800);

        assert!(world.contains_cell(WorldPoint { x: 400, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 400, y: 7 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 6 }));
        assert!(!world.contains_cell(WorldPoint { x: 60, y: 60 }));
    }

    #[test]
    fn contains_cell_ellipse_is_centered_on_offset_bounds() {
        let world: World = create_world(WorldShapeKind::Ellipse, 1000, 800, 800);

        assert!(world.contains_cell(WorldPoint { x: 1400, y: 7 }));
        assert!(!world.contains_cell(WorldPoint { x: 1400, y: 6 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 400 }));
    }

    #[test]
    fn contains_cell_ellipse_uses_both_semi_axes() {
        let world: World = create_world(WorldShapeKind::Ellipse, 0, 800, 400);

        assert!(world.contains_cell(WorldPoint { x: 400, y: 7 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 6 }));
        assert!(world.contains_cell(WorldPoint { x: 7, y: 200 }));
        assert!(!world.contains_cell(WorldPoint { x: 6, y: 200 }));
    }

    #[test]
    fn contains_cell_ellipse_follows_shrunk_subpixel_bounds() {
        let mut world: World = create_world(WorldShapeKind::Ellipse, 0, 800, 800);
        let is_inside_before_shrinking: bool = world.contains_cell(WorldPoint { x: 400, y: 145 });
        for _ in 0..1000 {
            world.shrink();
        }

        assert!(is_inside_before_shrinking);
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 145 }));
        assert!(world.contains_cell(WorldPoint { x: 400, y: 146 }));
    }

    #[test]
    fn is_outside_ellipse_counts_the_boundary_as_outside() {
        let bounds: WorldBounds = WorldBounds::from_pixel_size(800, 800);

        assert!(bounds.is_outside_ellipse(WorldPoint { x: 400, y: 0 }.to_subpixel_point()));
        assert!(!bounds.is_outside_ellipse(WorldPoint { x: 400, y: 1 }.to_subpixel_point()));
    }

    #[test]
    fn shrink_moves_each_edge_inward_by_half_a_step() {
        let mut world: World = create_world(WorldShapeKind::Rectangle, 0, 800, 800);
        world.shrink();

        assert_eq!(
            world.bounds,
            WorldBounds {
                left: Subpixels(143),
                top: Subpixels(143),
                width: Subpixels(819_200 - 286),
                height: Subpixels(819_200 - 286),
            },
        );
        assert_eq!(world.initial_bounds, WorldBounds::from_pixel_size(800, 800));
    }

    #[test]
    fn shrink_stops_once_a_dimension_reaches_the_minimum() {
        let mut world: World = World::new(
            WorldShapeKind::Rectangle,
            WorldBounds {
                left: Subpixels(0),
                top: Subpixels(0),
                width: Subpixels(205_000),
                height: Subpixels(400_000),
            },
        );
        world.shrink();
        world.shrink();

        assert_eq!(world.bounds.width, Subpixels(205_000 - 286));
        assert_eq!(world.bounds.height, Subpixels(400_000 - 286));
    }

    #[test]
    fn restore_initial_bounds_undoes_shrinking() {
        let mut world: World = create_world(WorldShapeKind::Ellipse, 0, 800, 800);
        world.shrink();
        world.restore_initial_bounds();

        assert_eq!(world.bounds, world.initial_bounds);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared world::world`
Expected: FAIL, compilation stops; the first error is `` error[E0425]: cannot find type `WorldShapeKind` in this scope ``.

- [ ] **Step 3: Write the implementation**

Replace the whole of `shared/src/world/world.rs` with:

```rust
use crate::geometry;
use crate::geometry::{SubpixelPoint, Subpixels, WorldPoint};

pub const SURVIVAL_SHRINK_MINIMUM_SUBPIXELS: i32 = 200 * geometry::SUBPIXELS_PER_PIXEL;
pub const SURVIVAL_SHRINK_SUBPIXELS_PER_TICK: i32 = 286;
const CELL_EXTENT_SUBPIXELS: i64 = (geometry::CELL_WIDTH_PIXELS * geometry::SUBPIXELS_PER_PIXEL) as i64;
const CELL_CORNER_SIGNS: [(i64, i64); 4] = [(-1, -1), (1, -1), (1, 1), (-1, 1)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldShapeKind {
    Rectangle,
    Ellipse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorldBounds {
    pub left: Subpixels,
    pub top: Subpixels,
    pub width: Subpixels,
    pub height: Subpixels,
}

impl WorldBounds {
    pub fn from_pixel_size(width_pixels: u32, height_pixels: u32) -> WorldBounds {
        WorldBounds {
            left: Subpixels(0),
            top: Subpixels(0),
            width: get_subpixels(width_pixels),
            height: get_subpixels(height_pixels),
        }
    }

    /// Against the ellipse inscribed in these bounds; a point on the ellipse counts as outside.
    pub fn is_outside_ellipse(&self, point: SubpixelPoint) -> bool {
        self.is_outside_ellipse_at(i64::from(point.x), i64::from(point.y))
    }

    fn is_outside_ellipse_at(&self, x: i64, y: i64) -> bool {
        let left: i128 = i128::from(self.left.0);
        let top: i128 = i128::from(self.top.0);
        let width: i128 = i128::from(self.width.0);
        let height: i128 = i128::from(self.height.0);
        let doubled_dx: i128 = 2 * i128::from(x) - (2 * left + width);
        let doubled_dy: i128 = 2 * i128::from(y) - (2 * top + height);

        doubled_dx * doubled_dx * height * height + doubled_dy * doubled_dy * width * width
            >= width * width * height * height
    }

    fn right(&self) -> i64 {
        i64::from(self.left.0) + i64::from(self.width.0)
    }

    fn bottom(&self) -> i64 {
        i64::from(self.top.0) + i64::from(self.height.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    pub shape: WorldShapeKind,
    pub bounds: WorldBounds,
    pub initial_bounds: WorldBounds,
}

impl World {
    pub fn new(shape: WorldShapeKind, bounds: WorldBounds) -> World {
        World {
            shape,
            bounds,
            initial_bounds: bounds,
        }
    }

    /// Whether the cell's box, centre ±6 px, lies strictly inside the world.
    pub fn contains_cell(&self, center: WorldPoint) -> bool {
        let center_x: i64 = i64::from(center.x) * i64::from(geometry::SUBPIXELS_PER_PIXEL);
        let center_y: i64 = i64::from(center.y) * i64::from(geometry::SUBPIXELS_PER_PIXEL);

        match self.shape {
            WorldShapeKind::Rectangle => self.rectangle_contains_cell(center_x, center_y),
            WorldShapeKind::Ellipse => self.ellipse_contains_cell(center_x, center_y),
        }
    }

    /// One tick of survival shrinking about the centre.
    pub fn shrink(&mut self) {
        let is_above_minimum: bool = self.bounds.width.0 > SURVIVAL_SHRINK_MINIMUM_SUBPIXELS
            && self.bounds.height.0 > SURVIVAL_SHRINK_MINIMUM_SUBPIXELS;
        if !is_above_minimum {
            return;
        }

        let half_step: i32 = SURVIVAL_SHRINK_SUBPIXELS_PER_TICK / 2;

        self.bounds = WorldBounds {
            left: Subpixels(self.bounds.left.0 + half_step),
            top: Subpixels(self.bounds.top.0 + half_step),
            width: Subpixels(self.bounds.width.0 - SURVIVAL_SHRINK_SUBPIXELS_PER_TICK),
            height: Subpixels(self.bounds.height.0 - SURVIVAL_SHRINK_SUBPIXELS_PER_TICK),
        };
    }

    pub fn restore_initial_bounds(&mut self) {
        self.bounds = self.initial_bounds;
    }

    fn rectangle_contains_cell(&self, center_x: i64, center_y: i64) -> bool {
        let is_outside: bool = center_x - CELL_EXTENT_SUBPIXELS <= i64::from(self.bounds.left.0)
            || center_x + CELL_EXTENT_SUBPIXELS >= self.bounds.right()
            || center_y - CELL_EXTENT_SUBPIXELS <= i64::from(self.bounds.top.0)
            || center_y + CELL_EXTENT_SUBPIXELS >= self.bounds.bottom();

        !is_outside
    }

    fn ellipse_contains_cell(&self, center_x: i64, center_y: i64) -> bool {
        CELL_CORNER_SIGNS.iter().all(|(sign_x, sign_y)| {
            let corner_x: i64 = center_x + sign_x * CELL_EXTENT_SUBPIXELS;
            let corner_y: i64 = center_y + sign_y * CELL_EXTENT_SUBPIXELS;

            !self.bounds.is_outside_ellipse_at(corner_x, corner_y)
        })
    }
}

fn get_subpixels(pixels: u32) -> Subpixels {
    let subpixels: i64 = i64::from(pixels) * i64::from(geometry::SUBPIXELS_PER_PIXEL);

    Subpixels(i32::try_from(subpixels).unwrap_or(i32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_world(shape: WorldShapeKind, left_pixels: i32, width_pixels: u32, height_pixels: u32) -> World {
        let mut bounds: WorldBounds = WorldBounds::from_pixel_size(width_pixels, height_pixels);
        bounds.left = Subpixels(left_pixels * geometry::SUBPIXELS_PER_PIXEL);

        World::new(shape, bounds)
    }

    #[test]
    fn contains_cell_rectangle_excludes_touching_the_border() {
        let world: World = create_world(WorldShapeKind::Rectangle, 0, 800, 800);

        assert!(!world.contains_cell(WorldPoint { x: 6, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 7, y: 400 }));
        assert!(!world.contains_cell(WorldPoint { x: 794, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 793, y: 400 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 6 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 794 }));
    }

    #[test]
    fn contains_cell_rectangle_follows_an_offset_origin() {
        let world: World = create_world(WorldShapeKind::Rectangle, 1000, 800, 800);

        assert!(!world.contains_cell(WorldPoint { x: 1006, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 1007, y: 400 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 400 }));
    }

    #[test]
    fn contains_cell_rectangle_follows_shrunk_subpixel_bounds() {
        let mut world: World = create_world(WorldShapeKind::Rectangle, 0, 800, 800);
        for _ in 0..1000 {
            world.shrink();
        }

        assert_eq!(world.bounds.left, Subpixels(143_000));
        assert!(!world.contains_cell(WorldPoint { x: 145, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 146, y: 400 }));
    }

    #[test]
    fn contains_cell_ellipse_tests_every_corner() {
        let world: World = create_world(WorldShapeKind::Ellipse, 0, 800, 800);

        assert!(world.contains_cell(WorldPoint { x: 400, y: 400 }));
        assert!(world.contains_cell(WorldPoint { x: 400, y: 7 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 6 }));
        assert!(!world.contains_cell(WorldPoint { x: 60, y: 60 }));
    }

    #[test]
    fn contains_cell_ellipse_is_centered_on_offset_bounds() {
        let world: World = create_world(WorldShapeKind::Ellipse, 1000, 800, 800);

        assert!(world.contains_cell(WorldPoint { x: 1400, y: 7 }));
        assert!(!world.contains_cell(WorldPoint { x: 1400, y: 6 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 400 }));
    }

    #[test]
    fn contains_cell_ellipse_uses_both_semi_axes() {
        let world: World = create_world(WorldShapeKind::Ellipse, 0, 800, 400);

        assert!(world.contains_cell(WorldPoint { x: 400, y: 7 }));
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 6 }));
        assert!(world.contains_cell(WorldPoint { x: 7, y: 200 }));
        assert!(!world.contains_cell(WorldPoint { x: 6, y: 200 }));
    }

    #[test]
    fn contains_cell_ellipse_follows_shrunk_subpixel_bounds() {
        let mut world: World = create_world(WorldShapeKind::Ellipse, 0, 800, 800);
        let is_inside_before_shrinking: bool = world.contains_cell(WorldPoint { x: 400, y: 145 });
        for _ in 0..1000 {
            world.shrink();
        }

        assert!(is_inside_before_shrinking);
        assert!(!world.contains_cell(WorldPoint { x: 400, y: 145 }));
        assert!(world.contains_cell(WorldPoint { x: 400, y: 146 }));
    }

    #[test]
    fn is_outside_ellipse_counts_the_boundary_as_outside() {
        let bounds: WorldBounds = WorldBounds::from_pixel_size(800, 800);

        assert!(bounds.is_outside_ellipse(WorldPoint { x: 400, y: 0 }.to_subpixel_point()));
        assert!(!bounds.is_outside_ellipse(WorldPoint { x: 400, y: 1 }.to_subpixel_point()));
    }

    #[test]
    fn shrink_moves_each_edge_inward_by_half_a_step() {
        let mut world: World = create_world(WorldShapeKind::Rectangle, 0, 800, 800);
        world.shrink();

        assert_eq!(
            world.bounds,
            WorldBounds {
                left: Subpixels(143),
                top: Subpixels(143),
                width: Subpixels(819_200 - 286),
                height: Subpixels(819_200 - 286),
            },
        );
        assert_eq!(world.initial_bounds, WorldBounds::from_pixel_size(800, 800));
    }

    #[test]
    fn shrink_stops_once_a_dimension_reaches_the_minimum() {
        let mut world: World = World::new(
            WorldShapeKind::Rectangle,
            WorldBounds {
                left: Subpixels(0),
                top: Subpixels(0),
                width: Subpixels(205_000),
                height: Subpixels(400_000),
            },
        );
        world.shrink();
        world.shrink();

        assert_eq!(world.bounds.width, Subpixels(205_000 - 286));
        assert_eq!(world.bounds.height, Subpixels(400_000 - 286));
    }

    #[test]
    fn restore_initial_bounds_undoes_shrinking() {
        let mut world: World = create_world(WorldShapeKind::Ellipse, 0, 800, 800);
        world.shrink();
        world.restore_initial_bounds();

        assert_eq!(world.bounds, world.initial_bounds);
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared world::world`
Expected: the `shared` library tests report `test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 34 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/src/lib.rs ./shared/src/world/mod.rs ./shared/src/world/world.rs
git commit -m "world bounds and border rule"
```

`git status` before the commit lists only the paths staged above.

### Task 9: Growth chance tables

**Files:**
- Create: `shared/src/organism/growth_chance_table.rs`
- Modify: `shared/src/organism/mod.rs`
- Test: inline `mod tests` in `shared/src/organism/growth_chance_table.rs`

Table lengths: birth `0..=1365`, `0..=525`, `0..=2448` and death `0..=2500`, `0..=1600`, `0..=4900`, 13,344 entries in total; the birth chance at the next `distance_squared` past each end is negative (Default: 0.00556 at 1365, -0.00425 at 1366).

- [ ] **Step 1: Write the failing test**

`shared/src/organism/mod.rs`, whole file:

```rust
pub mod cell_occupancy;
pub mod growth_chance_table;
pub mod organism;

pub use cell_occupancy::*;
pub use growth_chance_table::*;
pub use organism::*;
```

`shared/src/organism/growth_chance_table.rs`, tests only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const GROWTH_STATES: [GrowthStateKind; 3] = [
        GrowthStateKind::Default,
        GrowthStateKind::Compressed,
        GrowthStateKind::Extended,
    ];
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    const GOLDEN_TABLE_DIGEST: u64 = 0x75a8_9b54_1a01_86e5;

    fn get_tables_digest(tables: &GrowthChanceTables) -> u64 {
        let mut digest: u64 = FNV_OFFSET_BASIS;

        for growth_state in GROWTH_STATES {
            let table: &GrowthChanceTable = tables.get(growth_state);
            let thresholds: Vec<u64> = [table.birth_thresholds(), table.death_thresholds()].concat();

            for threshold in thresholds {
                for byte in threshold.to_le_bytes() {
                    digest ^= u64::from(byte);
                    digest = digest.wrapping_mul(FNV_PRIME);
                }
            }
        }

        digest
    }

    #[test]
    fn build_gives_the_designed_lengths() {
        let lengths: Vec<(usize, usize)> = GROWTH_STATES
            .iter()
            .map(|growth_state| {
                let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(*growth_state);

                (table.birth_thresholds().len(), table.death_thresholds().len())
            })
            .collect();
        let total_length: usize = lengths.iter().map(|(birth_length, death_length)| birth_length + death_length).sum();

        assert_eq!(lengths, vec![(1366, 2501), (526, 1601), (2449, 4901)]);
        assert_eq!(total_length, 13_344);
    }

    #[test]
    fn build_matches_the_golden_digest() {
        assert_eq!(get_tables_digest(&GROWTH_CHANCE_TABLES), GOLDEN_TABLE_DIGEST);
    }

    #[test]
    fn birth_table_last_distance_squared_is_the_last_non_negative_chance() {
        for growth_state in GROWTH_STATES {
            let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(growth_state);

            assert_ne!(table.birth_thresholds().last(), Some(&NEVER_PASSES_THRESHOLD));
            assert!(get_birth_chance(growth_state, growth_state.birth_table_last_distance_squared() + 1) < 0.0);
        }
    }

    #[test]
    fn birth_passes_always_at_the_cursor() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);

        assert_eq!(table.birth_thresholds()[0], ALWAYS_PASSES_THRESHOLD);
        assert!(table.birth_passes(0, u32::MAX));
    }

    #[test]
    fn birth_passes_never_past_the_table_end() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);

        assert!(!table.birth_passes(1366, 0));
        assert!(!table.birth_passes(1_000_000, 0));
    }

    #[test]
    fn birth_passes_below_the_threshold_only() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);
        let threshold: u64 = table.birth_thresholds()[36];
        let largest_passing_draw: u32 = u32::try_from(threshold - 1).unwrap();

        assert!(table.birth_passes(36, largest_passing_draw));
        assert!(!table.birth_passes(36, largest_passing_draw + 1));
    }

    #[test]
    fn death_passes_always_at_the_range() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);

        assert_eq!(table.death_thresholds()[2500], ALWAYS_PASSES_THRESHOLD);
        assert!(table.death_passes(2500, u32::MAX));
        assert!(table.death_passes(2501, u32::MAX));
    }

    #[test]
    fn death_passes_never_near_the_cursor() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);

        assert_eq!(table.death_thresholds()[0], NEVER_PASSES_THRESHOLD);
        assert!(!table.death_passes(0, 0));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared growth_chance_table`
Expected: FAIL, compilation stops; the first error is `` error[E0425]: cannot find type `GrowthStateKind` in this scope ``.

- [ ] **Step 3: Write the implementation**

Replace the whole of `shared/src/organism/growth_chance_table.rs` with:

```rust
use std::sync::LazyLock;

pub static GROWTH_CHANCE_TABLES: LazyLock<GrowthChanceTables> = LazyLock::new(GrowthChanceTables::build);

const DRAW_SPACE_SIZE: f64 = 4_294_967_296.0;
const ALWAYS_PASSES_THRESHOLD: u64 = 1 << 32;
const NEVER_PASSES_THRESHOLD: u64 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrowthStateKind {
    Default,
    Compressed,
    Extended,
}

impl GrowthStateKind {
    pub fn coefficient(self) -> f64 {
        match self {
            GrowthStateKind::Default => -27.5,
            GrowthStateKind::Compressed => -31.5,
            GrowthStateKind::Extended => -25.5,
        }
    }

    pub fn range_pixels(self) -> i64 {
        match self {
            GrowthStateKind::Default => 50,
            GrowthStateKind::Compressed => 40,
            GrowthStateKind::Extended => 70,
        }
    }

    pub fn range_squared(self) -> i64 {
        self.range_pixels() * self.range_pixels()
    }

    /// The largest `distance_squared` whose birth chance is not negative.
    pub fn birth_table_last_distance_squared(self) -> i64 {
        match self {
            GrowthStateKind::Default => 1365,
            GrowthStateKind::Compressed => 525,
            GrowthStateKind::Extended => 2448,
        }
    }
}

/// Exclusive u32 draw thresholds indexed by `distance_squared`: a draw `u` passes when `u < threshold`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrowthChanceTable {
    birth_thresholds: Vec<u64>,
    death_thresholds: Vec<u64>,
}

impl GrowthChanceTable {
    fn build(growth_state: GrowthStateKind) -> GrowthChanceTable {
        let birth_thresholds: Vec<u64> = (0..=growth_state.birth_table_last_distance_squared())
            .map(|distance_squared| get_threshold(get_birth_chance(growth_state, distance_squared)))
            .collect();
        let death_thresholds: Vec<u64> = (0..=growth_state.range_squared())
            .map(|distance_squared| get_threshold(get_death_chance(growth_state, distance_squared)))
            .collect();

        GrowthChanceTable {
            birth_thresholds,
            death_thresholds,
        }
    }

    pub fn birth_thresholds(&self) -> &[u64] {
        &self.birth_thresholds
    }

    pub fn death_thresholds(&self) -> &[u64] {
        &self.death_thresholds
    }

    /// Never passes past the end of the table.
    pub fn birth_passes(&self, distance_squared: i64, draw: u32) -> bool {
        let threshold: u64 = get_entry(&self.birth_thresholds, distance_squared).unwrap_or(NEVER_PASSES_THRESHOLD);

        u64::from(draw) < threshold
    }

    /// Always passes past the end of the table, where death is forced.
    pub fn death_passes(&self, distance_squared: i64, draw: u32) -> bool {
        let threshold: u64 = get_entry(&self.death_thresholds, distance_squared).unwrap_or(ALWAYS_PASSES_THRESHOLD);

        u64::from(draw) < threshold
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrowthChanceTables {
    default: GrowthChanceTable,
    compressed: GrowthChanceTable,
    extended: GrowthChanceTable,
}

impl GrowthChanceTables {
    fn build() -> GrowthChanceTables {
        GrowthChanceTables {
            default: GrowthChanceTable::build(GrowthStateKind::Default),
            compressed: GrowthChanceTable::build(GrowthStateKind::Compressed),
            extended: GrowthChanceTable::build(GrowthStateKind::Extended),
        }
    }

    pub fn get(&self, growth_state: GrowthStateKind) -> &GrowthChanceTable {
        match growth_state {
            GrowthStateKind::Default => &self.default,
            GrowthStateKind::Compressed => &self.compressed,
            GrowthStateKind::Extended => &self.extended,
        }
    }
}

/// The original's `coefficient * ln(r + 1) + 100`, in percent.
fn get_birth_chance(growth_state: GrowthStateKind, distance_squared: i64) -> f64 {
    let distance: f64 = libm::sqrt(distance_squared as f64);

    growth_state.coefficient() * libm::log(distance + 1.0) + 100.0
}

/// The original's `coefficient * ln(range + 1 - r) + 100`, in percent.
fn get_death_chance(growth_state: GrowthStateKind, distance_squared: i64) -> f64 {
    let distance: f64 = libm::sqrt(distance_squared as f64);
    let range: f64 = growth_state.range_pixels() as f64;

    growth_state.coefficient() * libm::log(range + 1.0 - distance) + 100.0
}

/// The original passes when `random * 100 <= chance`; scaled to u32 draws that is `u <= floor(chance / 100 * 2^32)`.
fn get_threshold(chance: f64) -> u64 {
    if chance < 0.0 {
        return NEVER_PASSES_THRESHOLD;
    }

    let largest_passing_draw: f64 = libm::floor(chance / 100.0 * DRAW_SPACE_SIZE);

    (largest_passing_draw as u64 + 1).min(ALWAYS_PASSES_THRESHOLD)
}

fn get_entry(thresholds: &[u64], distance_squared: i64) -> Option<u64> {
    let index: usize = usize::try_from(distance_squared).ok()?;

    thresholds.get(index).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROWTH_STATES: [GrowthStateKind; 3] = [
        GrowthStateKind::Default,
        GrowthStateKind::Compressed,
        GrowthStateKind::Extended,
    ];
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    const GOLDEN_TABLE_DIGEST: u64 = 0x75a8_9b54_1a01_86e5;

    fn get_tables_digest(tables: &GrowthChanceTables) -> u64 {
        let mut digest: u64 = FNV_OFFSET_BASIS;

        for growth_state in GROWTH_STATES {
            let table: &GrowthChanceTable = tables.get(growth_state);
            let thresholds: Vec<u64> = [table.birth_thresholds(), table.death_thresholds()].concat();

            for threshold in thresholds {
                for byte in threshold.to_le_bytes() {
                    digest ^= u64::from(byte);
                    digest = digest.wrapping_mul(FNV_PRIME);
                }
            }
        }

        digest
    }

    #[test]
    fn build_gives_the_designed_lengths() {
        let lengths: Vec<(usize, usize)> = GROWTH_STATES
            .iter()
            .map(|growth_state| {
                let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(*growth_state);

                (table.birth_thresholds().len(), table.death_thresholds().len())
            })
            .collect();
        let total_length: usize = lengths.iter().map(|(birth_length, death_length)| birth_length + death_length).sum();

        assert_eq!(lengths, vec![(1366, 2501), (526, 1601), (2449, 4901)]);
        assert_eq!(total_length, 13_344);
    }

    #[test]
    fn build_matches_the_golden_digest() {
        assert_eq!(get_tables_digest(&GROWTH_CHANCE_TABLES), GOLDEN_TABLE_DIGEST);
    }

    #[test]
    fn birth_table_last_distance_squared_is_the_last_non_negative_chance() {
        for growth_state in GROWTH_STATES {
            let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(growth_state);

            assert_ne!(table.birth_thresholds().last(), Some(&NEVER_PASSES_THRESHOLD));
            assert!(get_birth_chance(growth_state, growth_state.birth_table_last_distance_squared() + 1) < 0.0);
        }
    }

    #[test]
    fn birth_passes_always_at_the_cursor() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);

        assert_eq!(table.birth_thresholds()[0], ALWAYS_PASSES_THRESHOLD);
        assert!(table.birth_passes(0, u32::MAX));
    }

    #[test]
    fn birth_passes_never_past_the_table_end() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);

        assert!(!table.birth_passes(1366, 0));
        assert!(!table.birth_passes(1_000_000, 0));
    }

    #[test]
    fn birth_passes_below_the_threshold_only() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);
        let threshold: u64 = table.birth_thresholds()[36];
        let largest_passing_draw: u32 = u32::try_from(threshold - 1).unwrap();

        assert!(table.birth_passes(36, largest_passing_draw));
        assert!(!table.birth_passes(36, largest_passing_draw + 1));
    }

    #[test]
    fn death_passes_always_at_the_range() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);

        assert_eq!(table.death_thresholds()[2500], ALWAYS_PASSES_THRESHOLD);
        assert!(table.death_passes(2500, u32::MAX));
        assert!(table.death_passes(2501, u32::MAX));
    }

    #[test]
    fn death_passes_never_near_the_cursor() {
        let table: &GrowthChanceTable = GROWTH_CHANCE_TABLES.get(GrowthStateKind::Default);

        assert_eq!(table.death_thresholds()[0], NEVER_PASSES_THRESHOLD);
        assert!(!table.death_passes(0, 0));
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared growth_chance_table`
Expected: the `shared` library tests report `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 45 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/src/organism/growth_chance_table.rs ./shared/src/organism/mod.rs
git commit -m "growth chance tables"
```

`git status` before the commit lists only the paths staged above.

### Task 10: Game settings and mode codes

**Files:**
- Create: `shared/src/game/game_settings.rs`
- Modify: `shared/src/game/mod.rs`
- Test: inline `mod tests` in `shared/src/game/game_settings.rs`

- [ ] **Step 1: Write the failing test**

`shared/src/game/mod.rs`, whole file:

```rust
pub mod game_settings;
pub mod tick;

pub use game_settings::*;
pub use tick::*;
```

`shared/src/game/game_settings.rs`, tests only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_from_reads_every_code() {
        for mode in [GameModeKind::FreeForAll, GameModeKind::Skirmish, GameModeKind::Survival] {
            assert_eq!(GameModeKind::try_from(mode.code()), Ok(mode));
        }
    }

    #[test]
    fn try_from_rejects_an_unknown_code() {
        assert_eq!(
            GameModeKind::try_from("ctf"),
            Err(UnknownGameModeCode {
                code: String::from("ctf"),
            }),
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared game_settings`
Expected: FAIL, compilation stops; the first error is `` error[E0422]: cannot find struct, variant or union type `UnknownGameModeCode` in this scope ``.

- [ ] **Step 3: Write the implementation**

Replace the whole of `shared/src/game/game_settings.rs` with:

```rust
use crate::world::WorldShapeKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSettings {
    pub title: String,
    pub mode: GameModeKind,
    pub world_shape: WorldShapeKind,
    pub world_width_pixels: u32,
    pub world_height_pixels: u32,
    /// Some only in survival.
    pub player_minimum: Option<u8>,
    /// Alive organisms, not members.
    pub player_cap: u8,
    /// Some only in skirmish.
    pub team_count: Option<u8>,
    pub leaderboard_length: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameModeKind {
    FreeForAll,
    Skirmish,
    Survival,
}

impl GameModeKind {
    pub fn code(self) -> &'static str {
        match self {
            GameModeKind::FreeForAll => "ffa",
            GameModeKind::Skirmish => "skm",
            GameModeKind::Survival => "srv",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownGameModeCode {
    pub code: String,
}

impl TryFrom<&str> for GameModeKind {
    type Error = UnknownGameModeCode;

    fn try_from(code: &str) -> Result<GameModeKind, UnknownGameModeCode> {
        match code {
            "ffa" => Ok(GameModeKind::FreeForAll),
            "skm" => Ok(GameModeKind::Skirmish),
            "srv" => Ok(GameModeKind::Survival),
            _ => Err(UnknownGameModeCode {
                code: String::from(code),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_from_reads_every_code() {
        for mode in [GameModeKind::FreeForAll, GameModeKind::Skirmish, GameModeKind::Survival] {
            assert_eq!(GameModeKind::try_from(mode.code()), Ok(mode));
        }
    }

    #[test]
    fn try_from_rejects_an_unknown_code() {
        assert_eq!(
            GameModeKind::try_from("ctf"),
            Err(UnknownGameModeCode {
                code: String::from("ctf"),
            }),
        );
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared game_settings`
Expected: the `shared` library tests report `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 53 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/src/game/game_settings.rs ./shared/src/game/mod.rs
git commit -m "game settings and mode codes"
```

`git status` before the commit lists only the paths staged above.

### Task 11: Game state with members, score and round state

**Files:**
- Create: `shared/src/round/mod.rs`
- Create: `shared/src/round/round.rs`
- Create: `shared/src/member/scoreboard.rs`
- Create: `shared/src/game/test_fixture.rs`
- Create: `shared/src/game/game_state.rs`
- Modify: `shared/src/lib.rs`
- Modify: `shared/src/member/mod.rs`
- Modify: `shared/src/member/member.rs`
- Modify: `shared/src/game/mod.rs`
- Test: inline `mod tests` in `shared/src/game/game_state.rs`

`RoundState` and `RoundPhase` are data only; the transitions are phase 3. `Score` is the whole of `scoreboard.rs` until the leaderboard rows arrive in phase 3.

- [ ] **Step 1: Add the data types the test needs**

`shared/src/lib.rs`, whole file:

```rust
//! Everything the server and the clients must agree on, plus platform-independent client logic.

pub mod ability;
pub mod game;
pub mod geometry;
pub mod member;
pub mod organism;
pub mod random;
pub mod round;
pub mod world;
```

`shared/src/round/mod.rs`:

```rust
pub mod round;

pub use round::*;
```

`shared/src/round/round.rs`:

```rust
use crate::game::Tick;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoundState {
    pub phase: RoundPhase,
    pub phase_started_at: Tick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundPhase {
    Waiting,
    PreRound,
    Playing,
    PostRound,
}
```

`shared/src/member/scoreboard.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Score {
    pub kills: u32,
    pub deaths: u32,
    pub wins: u32,
}

impl Score {
    pub fn zero() -> Score {
        Score {
            kills: 0,
            deaths: 0,
            wins: 0,
        }
    }
}
```

`shared/src/member/mod.rs`, whole file:

```rust
pub mod member;
pub mod scoreboard;

pub use member::*;
pub use scoreboard::*;
```

`shared/src/member/member.rs`: insert at the top of the file, above `/// Never reused within a game.`, followed by one blank line:

```rust
use crate::ability::Loadout;
use crate::member::Score;
use crate::organism::Organism;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub member_id: MemberId,
    pub screen_name: String,
    pub role: MemberRoleKind,
    /// Some for a Participant.
    pub loadout: Option<Loadout>,
    /// Some only in skirmish.
    pub team: Option<TeamKind>,
    pub score: Score,
    /// None when dead, awaiting spawn, or spectating.
    pub organism: Option<Organism>,
}
```

- [ ] **Step 2: Write the fixtures and the failing test**

`shared/src/game/mod.rs`, whole file:

```rust
pub mod game_settings;
pub mod game_state;
// Fixtures shared by the unit tests of several modules; absent from every non-test build.
#[cfg(test)]
pub mod test_fixture;
pub mod tick;

pub use game_settings::*;
pub use game_state::*;
pub use tick::*;
```

`shared/src/game/test_fixture.rs`:

```rust
use crate::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use crate::game::{GameModeKind, GameSettings, GameState};
use crate::geometry::WorldPoint;
use crate::member::{Appearance, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind};
use crate::organism::Organism;
use crate::world::WorldShapeKind;

pub const FIXTURE_SEED: u64 = 0x5eed;

pub fn create_settings(mode: GameModeKind, world_shape: WorldShapeKind, world_size_pixels: u32) -> GameSettings {
    let player_minimum: Option<u8> = match mode {
        GameModeKind::Survival => Some(2),
        GameModeKind::FreeForAll | GameModeKind::Skirmish => None,
    };
    let team_count: Option<u8> = match mode {
        GameModeKind::Skirmish => Some(2),
        GameModeKind::FreeForAll | GameModeKind::Survival => None,
    };

    GameSettings {
        title: String::from("Fixture game"),
        mode,
        world_shape,
        world_width_pixels: world_size_pixels,
        world_height_pixels: world_size_pixels,
        player_minimum,
        player_cap: 8,
        team_count,
        leaderboard_length: 10,
    }
}

pub fn create_state(mode: GameModeKind, world_shape: WorldShapeKind, world_size_pixels: u32) -> GameState {
    GameState::new(create_settings(mode, world_shape, world_size_pixels), FIXTURE_SEED)
}

pub fn create_loadout() -> Loadout {
    Loadout {
        appearance: Appearance {
            color: OrganismColorKind::Ocean,
            skin: SkinKind::Circles,
        },
        first: FirstAbilityKind::Extend,
        second: SecondAbilityKind::Immortality,
        third: ThirdAbilityKind::Neutralize,
    }
}

pub fn create_participant(member_id: MemberId) -> Member {
    Member {
        member_id,
        screen_name: format!("player {}", member_id.0),
        role: MemberRoleKind::Participant,
        loadout: Some(create_loadout()),
        team: None,
        score: Score::zero(),
        organism: None,
    }
}

pub fn create_participant_with_organism(member_id: MemberId, position: WorldPoint) -> Member {
    let mut member: Member = create_participant(member_id);
    member.organism = Some(Organism::new(position));

    member
}

pub fn get_organism(state: &GameState, member_id: MemberId) -> &Organism {
    state.members[&member_id].organism.as_ref().unwrap()
}

pub fn get_organism_mut(state: &mut GameState, member_id: MemberId) -> &mut Organism {
    state.members.get_mut(&member_id).unwrap().organism.as_mut().unwrap()
}
```

`shared/src/game/game_state.rs`, tests only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::geometry::{Subpixels, WorldPoint};
    use crate::world::WorldShapeKind;

    #[test]
    fn new_starts_empty_at_tick_zero() {
        let settings: GameSettings =
            test_fixture::create_settings(GameModeKind::FreeForAll, WorldShapeKind::Ellipse, 800);
        let state: GameState = GameState::new(settings, 17);

        assert_eq!(state.tick, Tick(0));
        assert!(state.members.is_empty());
        assert_eq!(state.next_member_id, MemberId(0));
        assert_eq!(state.rng, Pcg32::from_seed(17));
        assert_eq!(state.round, None);
    }

    #[test]
    fn new_builds_the_world_from_the_settings() {
        let mut settings: GameSettings =
            test_fixture::create_settings(GameModeKind::FreeForAll, WorldShapeKind::Ellipse, 1200);
        settings.world_height_pixels = 700;
        let state: GameState = GameState::new(settings, 0);

        assert_eq!(state.world.shape, WorldShapeKind::Ellipse);
        assert_eq!(state.world.bounds.width, Subpixels(1200 * 1024));
        assert_eq!(state.world.bounds.height, Subpixels(700 * 1024));
        assert_eq!(state.world.initial_bounds, state.world.bounds);
    }

    #[test]
    fn new_waits_for_a_round_only_in_survival() {
        let survival_state: GameState =
            test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Rectangle, 800);
        let skirmish_state: GameState =
            test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);

        assert_eq!(
            survival_state.round,
            Some(RoundState {
                phase: RoundPhase::Waiting,
                phase_started_at: Tick(0),
            }),
        );
        assert_eq!(skirmish_state.round, None);
    }

    #[test]
    fn alive_organism_count_counts_members_with_an_organism() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.members.insert(
            MemberId(0),
            test_fixture::create_participant_with_organism(MemberId(0), WorldPoint { x: 100, y: 100 }),
        );
        state.members.insert(MemberId(1), test_fixture::create_participant(MemberId(1)));

        assert_eq!(state.alive_organism_count(), 1);
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p shared game_state`
Expected: FAIL, compilation stops; the first error is `` error[E0432]: unresolved import `crate::game::GameState` ``.

- [ ] **Step 4: Write the implementation**

Replace the whole of `shared/src/game/game_state.rs` with:

```rust
use std::collections::BTreeMap;

use crate::game::{GameModeKind, GameSettings, Tick};
use crate::member::{Member, MemberId};
use crate::random::Pcg32;
use crate::round::{RoundPhase, RoundState};
use crate::world::{World, WorldBounds};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameState {
    pub tick: Tick,
    /// Immutable after creation.
    pub settings: GameSettings,
    pub rng: Pcg32,
    pub world: World,
    /// Some only in survival.
    pub round: Option<RoundState>,
    pub members: BTreeMap<MemberId, Member>,
    /// One past the highest member id joined so far.
    pub next_member_id: MemberId,
}

impl GameState {
    /// Tick 0, no members.
    pub fn new(settings: GameSettings, seed: u64) -> GameState {
        let bounds: WorldBounds =
            WorldBounds::from_pixel_size(settings.world_width_pixels, settings.world_height_pixels);
        let round: Option<RoundState> = match settings.mode {
            GameModeKind::Survival => Some(RoundState {
                phase: RoundPhase::Waiting,
                phase_started_at: Tick(0),
            }),
            GameModeKind::FreeForAll | GameModeKind::Skirmish => None,
        };

        GameState {
            tick: Tick(0),
            world: World::new(settings.world_shape, bounds),
            settings,
            rng: Pcg32::from_seed(seed),
            round,
            members: BTreeMap::new(),
            next_member_id: MemberId(0),
        }
    }

    pub fn alive_organism_count(&self) -> u32 {
        let alive_organism_count: usize = self.members.values().filter(|member| member.organism.is_some()).count();

        u32::try_from(alive_organism_count).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::geometry::{Subpixels, WorldPoint};
    use crate::world::WorldShapeKind;

    #[test]
    fn new_starts_empty_at_tick_zero() {
        let settings: GameSettings =
            test_fixture::create_settings(GameModeKind::FreeForAll, WorldShapeKind::Ellipse, 800);
        let state: GameState = GameState::new(settings, 17);

        assert_eq!(state.tick, Tick(0));
        assert!(state.members.is_empty());
        assert_eq!(state.next_member_id, MemberId(0));
        assert_eq!(state.rng, Pcg32::from_seed(17));
        assert_eq!(state.round, None);
    }

    #[test]
    fn new_builds_the_world_from_the_settings() {
        let mut settings: GameSettings =
            test_fixture::create_settings(GameModeKind::FreeForAll, WorldShapeKind::Ellipse, 1200);
        settings.world_height_pixels = 700;
        let state: GameState = GameState::new(settings, 0);

        assert_eq!(state.world.shape, WorldShapeKind::Ellipse);
        assert_eq!(state.world.bounds.width, Subpixels(1200 * 1024));
        assert_eq!(state.world.bounds.height, Subpixels(700 * 1024));
        assert_eq!(state.world.initial_bounds, state.world.bounds);
    }

    #[test]
    fn new_waits_for_a_round_only_in_survival() {
        let survival_state: GameState =
            test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Rectangle, 800);
        let skirmish_state: GameState =
            test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);

        assert_eq!(
            survival_state.round,
            Some(RoundState {
                phase: RoundPhase::Waiting,
                phase_started_at: Tick(0),
            }),
        );
        assert_eq!(skirmish_state.round, None);
    }

    #[test]
    fn alive_organism_count_counts_members_with_an_organism() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.members.insert(
            MemberId(0),
            test_fixture::create_participant_with_organism(MemberId(0), WorldPoint { x: 100, y: 100 }),
        );
        state.members.insert(MemberId(1), test_fixture::create_participant(MemberId(1)));

        assert_eq!(state.alive_organism_count(), 1);
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p shared game_state`
Expected: the `shared` library tests report `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 55 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 6: Commit**

```sh
git status
git add ./shared/src/game/game_state.rs ./shared/src/game/mod.rs ./shared/src/game/test_fixture.rs ./shared/src/lib.rs ./shared/src/member/member.rs ./shared/src/member/mod.rs ./shared/src/member/scoreboard.rs ./shared/src/round/mod.rs ./shared/src/round/round.rs
git commit -m "game state with members, score and round state"
```

`git status` before the commit lists only the paths staged above.

### Task 12: Birth phase

**Files:**
- Create: `shared/src/organism/growth.rs`
- Modify: `shared/src/organism/mod.rs`
- Test: inline `mod tests` in `shared/src/organism/growth.rs`

The fixture world is a 300 px rectangle. Draw counts the tests assert: a cell at (12, 150) has its left site at (6, 150), outside (`6 - 6 <= 0`), so 3 draws; organisms at (100, 100) and (112, 103) each lose one site to a collision across their unaligned lattices (|dx| = 6, |dy| = 3), so 3 + 3 draws; cells (0, 0) and (2, 0) emit site (1, 0) twice, so 8 draws, the second roll of (1, 0) finding it already born.

- [ ] **Step 1: Write the failing test**

`shared/src/organism/mod.rs`, whole file:

```rust
pub mod cell_occupancy;
pub mod growth;
pub mod growth_chance_table;
pub mod organism;

pub use cell_occupancy::*;
pub use growth::*;
pub use growth_chance_table::*;
pub use organism::*;
```

`shared/src/organism/growth.rs`, tests only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{AbilityPhase, FirstAbilityKind};
    use crate::game::GameModeKind;
    use crate::game::test_fixture;
    use crate::member::TeamKind;
    use crate::organism::Organism;
    use crate::world::WorldShapeKind;

    const FIRST_MEMBER_ID: MemberId = MemberId(0);
    const SECOND_MEMBER_ID: MemberId = MemberId(1);

    fn create_state_with_organisms(positions: &[WorldPoint]) -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 300);

        for (index, position) in positions.iter().enumerate() {
            let member_id: MemberId = MemberId(u32::try_from(index).unwrap());
            state.members.insert(
                member_id,
                test_fixture::create_participant_with_organism(member_id, *position),
            );
        }

        state
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
        let expected_births: Vec<LatticeCoordinate> = test_fixture::get_organism(&state, FIRST_MEMBER_ID)
            .adjacent_sites()
            .into_iter()
            .filter(|_| table.birth_passes(36, expected_rng.next_u32()))
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
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared organism::growth::`
Expected: FAIL, compilation stops; the first error is `` error[E0425]: cannot find type `MemberId` in this scope ``.

- [ ] **Step 3: Write the implementation**

Replace the whole of `shared/src/organism/growth.rs` with:

```rust
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

/// Every organism's cell centres, bucketed by `floor(center / CELL_WIDTH_PIXELS)`.
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
    use crate::world::WorldShapeKind;

    const FIRST_MEMBER_ID: MemberId = MemberId(0);
    const SECOND_MEMBER_ID: MemberId = MemberId(1);

    fn create_state_with_organisms(positions: &[WorldPoint]) -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 300);

        for (index, position) in positions.iter().enumerate() {
            let member_id: MemberId = MemberId(u32::try_from(index).unwrap());
            state.members.insert(
                member_id,
                test_fixture::create_participant_with_organism(member_id, *position),
            );
        }

        state
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
        let expected_births: Vec<LatticeCoordinate> = test_fixture::get_organism(&state, FIRST_MEMBER_ID)
            .adjacent_sites()
            .into_iter()
            .filter(|_| table.birth_passes(36, expected_rng.next_u32()))
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
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared organism::growth::`
Expected: the `shared` library tests report `test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 59 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/src/organism/growth.rs ./shared/src/organism/mod.rs
git commit -m "birth phase"
```

`git status` before the commit lists only the paths staged above.

### Task 13: Natural death phase

**Files:**
- Modify: `shared/src/organism/growth.rs`
- Test: inline `mod tests` in `shared/src/organism/growth.rs`

- [ ] **Step 1: Write the failing tests**

In `shared/src/organism/growth.rs`, append these tests at the end of `mod tests`, after a blank line and before its closing brace:

```rust
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared organism::growth::`
Expected: FAIL, compilation stops; the first error is `` error[E0425]: cannot find function `run_natural_death_phase` in this scope ``.

- [ ] **Step 3: Write the implementation**

In `shared/src/organism/growth.rs`, insert directly above `fn get_bucket(center: WorldPoint) -> (i32, i32) {`:

```rust
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared organism::growth::`
Expected: the `shared` library tests report `test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 59 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/src/organism/growth.rs
git commit -m "natural death phase"
```

`git status` before the commit lists only the paths staged above.

### Task 14: Spawn search and organism placement

**Files:**
- Create: `shared/src/game/simulation_event.rs`
- Create: `shared/src/ability/ability_constants.rs`
- Create: `shared/src/organism/spawn.rs`
- Modify: `shared/src/game/mod.rs`
- Modify: `shared/src/ability/mod.rs`
- Modify: `shared/src/organism/mod.rs`
- Test: inline `mod tests` in `shared/src/organism/spawn.rs`

Radii the hazard tests rely on: 24 px gives `576 * 1024² = 603_979_776 <= 634_933_739` (inside the spore secretion) and 25 px gives `655_360_000` (outside); 12 px gives `150_994_944 <= 158_733_434` (inside the shot secretion) and 13 px gives `177_209_344` (outside). The covering organism of the attempt-limit test has cells every 6 px from 53 to 251 on both axes, so every candidate in `[53, 247)` lies within 3 px of a cell centre.

- [ ] **Step 1: Add the event and constant types the test needs**

`shared/src/game/mod.rs`, whole file:

```rust
pub mod game_settings;
pub mod game_state;
pub mod simulation_event;
// Fixtures shared by the unit tests of several modules; absent from every non-test build.
#[cfg(test)]
pub mod test_fixture;
pub mod tick;

pub use game_settings::*;
pub use game_state::*;
pub use simulation_event::*;
pub use tick::*;
```

`shared/src/game/simulation_event.rs`:

```rust
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpawnRejectionKind {
    PositionNotFound,
    CapReached,
    RoundInProgress,
    AlreadyAlive,
}
```

`shared/src/ability/mod.rs`, whole file:

```rust
pub mod ability_constants;
pub mod ability_model;

pub use ability_constants::*;
pub use ability_model::*;
```

`shared/src/ability/ability_constants.rs`:

```rust
/// Floor of `72 * 2.9²` px² in subpixel²; the comparison is `<=`.
pub const SPORE_SECRETION_RADIUS_SQUARED_SUBPIXELS: i64 = 634_933_739;
/// Floor of `151.38` px² in subpixel²; the comparison is `<=`.
pub const SHOT_SECRETION_RADIUS_SQUARED_SUBPIXELS: i64 = 158_733_434;
/// Neutralize and toxin, 60 px; the comparison is `<=`.
pub const FIELD_RADIUS_SQUARED_PIXELS: i64 = 3600;
```

- [ ] **Step 2: Write the failing test**

`shared/src/organism/mod.rs`, whole file:

```rust
pub mod cell_occupancy;
pub mod growth;
pub mod growth_chance_table;
pub mod organism;
pub mod spawn;

pub use cell_occupancy::*;
pub use growth::*;
pub use growth_chance_table::*;
pub use organism::*;
pub use spawn::*;
```

`shared/src/organism/spawn.rs`, tests only for now:

```rust
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
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p shared organism::spawn`
Expected: FAIL, compilation stops; the first error is `` error[E0425]: cannot find type `MemberId` in this scope ``.

- [ ] **Step 4: Write the implementation**

Replace the whole of `shared/src/organism/spawn.rs` with:

```rust
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
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p shared organism::spawn`
Expected: the `shared` library tests report `test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 76 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 6: Commit**

```sh
git status
git add ./shared/src/ability/ability_constants.rs ./shared/src/ability/mod.rs ./shared/src/game/mod.rs ./shared/src/game/simulation_event.rs ./shared/src/organism/mod.rs ./shared/src/organism/spawn.rs
git commit -m "spawn search and organism placement"
```

`git status` before the commit lists only the paths staged above.

### Task 15: Step bundle checks and membership events

**Files:**
- Create: `shared/src/game/input_bundle.rs`
- Create: `shared/src/game/step.rs`
- Modify: `shared/src/game/mod.rs`
- Test: inline `mod tests` in `shared/src/game/step.rs`

This task covers tick-order phase 1 (membership events, strictly in bundle order) and the final tick assignment; Task 16 adds the remaining phase-2 phases.

- [ ] **Step 1: Add the bundle types the test needs**

`shared/src/game/mod.rs`, whole file:

```rust
pub mod game_settings;
pub mod game_state;
pub mod input_bundle;
pub mod simulation_event;
pub mod step;
// Fixtures shared by the unit tests of several modules; absent from every non-test build.
#[cfg(test)]
pub mod test_fixture;
pub mod tick;

pub use game_settings::*;
pub use game_state::*;
pub use input_bundle::*;
pub use simulation_event::*;
pub use step::*;
pub use tick::*;
```

`shared/src/game/input_bundle.rs`:

```rust
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
```

- [ ] **Step 2: Write the failing test**

`shared/src/game/step.rs`, tests only for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::member::{MemberRoleKind, OrganismColorKind, SkinKind, TeamKind};
    use crate::world::WorldShapeKind;

    fn create_bundle(tick: u32, member_events: Vec<MemberEvent>) -> InputBundle {
        InputBundle {
            tick: Tick(tick),
            member_events,
            player_inputs: Vec::new(),
        }
    }

    fn create_joined_event(member_id: MemberId, team: Option<TeamKind>) -> MemberEvent {
        MemberEvent::Joined {
            member_id,
            screen_name: format!("player {}", member_id.0),
            role: MemberRoleKind::Participant,
            loadout: Some(test_fixture::create_loadout()),
            team,
        }
    }

    fn create_state() -> GameState {
        test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800)
    }

    #[test]
    fn step_rejects_a_tick_mismatch_untouched() {
        let mut state: GameState = create_state();
        let state_before: GameState = state.clone();

        let result: Result<Vec<SimulationEvent>, StepError> = step(
            &mut state,
            &create_bundle(2, vec![create_joined_event(MemberId(0), None)]),
        );

        assert_eq!(
            result,
            Err(StepError::TickMismatch {
                expected: Tick(1),
                received: Tick(2),
            }),
        );
        assert_eq!(state, state_before);
    }

    #[test]
    fn step_rejects_a_member_id_below_the_next_untouched() {
        let mut state: GameState = create_state();
        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(3), None)]),
        )
        .unwrap();
        let state_before: GameState = state.clone();

        let result: Result<Vec<SimulationEvent>, StepError> = step(
            &mut state,
            &create_bundle(
                2,
                vec![
                    create_joined_event(MemberId(5), None),
                    create_joined_event(MemberId(5), None),
                ],
            ),
        );

        assert_eq!(
            result,
            Err(StepError::MemberIdOutOfOrder {
                minimum: MemberId(6),
                received: MemberId(5),
            }),
        );
        assert_eq!(state, state_before);
        assert_eq!(
            step(
                &mut state,
                &create_bundle(2, vec![create_joined_event(MemberId(2), None)]),
            ),
            Err(StepError::MemberIdOutOfOrder {
                minimum: MemberId(4),
                received: MemberId(2),
            }),
        );
    }

    #[test]
    fn step_advances_the_tick() {
        let mut state: GameState = create_state();

        assert_eq!(step(&mut state, &create_bundle(1, Vec::new())), Ok(Vec::new()));
        assert_eq!(state.tick, Tick(1));
    }

    #[test]
    fn step_joined_adds_a_member_without_an_organism() {
        let mut state: GameState = create_state();

        let simulation_events: Vec<SimulationEvent> = step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(4), None)]),
        )
        .unwrap();

        assert_eq!(
            simulation_events,
            vec![SimulationEvent::MemberJoined { member_id: MemberId(4) }],
        );
        assert_eq!(state.next_member_id, MemberId(5));
        assert_eq!(state.members[&MemberId(4)].screen_name, "player 4");
        assert_eq!(state.members[&MemberId(4)].organism, None);
        assert_eq!(state.members[&MemberId(4)].score, Score::zero());
    }

    #[test]
    fn step_joined_forces_the_team_color() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);

        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(0), Some(TeamKind::Red))]),
        )
        .unwrap();

        assert_eq!(
            state.members[&MemberId(0)].loadout.unwrap().appearance.color,
            OrganismColorKind::Fire,
        );
    }

    #[test]
    fn step_left_removes_the_member() {
        let mut state: GameState = create_state();
        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(0), None)]),
        )
        .unwrap();

        let simulation_events: Vec<SimulationEvent> = step(
            &mut state,
            &create_bundle(
                2,
                vec![
                    MemberEvent::Left { member_id: MemberId(0) },
                    MemberEvent::Left { member_id: MemberId(7) },
                ],
            ),
        )
        .unwrap();

        assert_eq!(
            simulation_events,
            vec![SimulationEvent::MemberLeft { member_id: MemberId(0) }],
        );
        assert!(state.members.is_empty());
        assert_eq!(state.next_member_id, MemberId(1));
    }

    #[test]
    fn step_spawn_requested_follows_its_join_in_bundle_order() {
        let mut state: GameState = create_state();

        let simulation_events: Vec<SimulationEvent> = step(
            &mut state,
            &create_bundle(
                1,
                vec![
                    create_joined_event(MemberId(0), None),
                    MemberEvent::SpawnRequested {
                        member_id: MemberId(0),
                        loadout: test_fixture::create_loadout(),
                        team: None,
                    },
                ],
            ),
        )
        .unwrap();

        let cursor: WorldPoint = test_fixture::get_organism(&state, MemberId(0)).cursor;
        assert_eq!(
            simulation_events,
            vec![
                SimulationEvent::MemberJoined { member_id: MemberId(0) },
                SimulationEvent::OrganismSpawned {
                    member_id: MemberId(0),
                    cursor,
                },
            ],
        );
    }

    #[test]
    fn step_appearance_changed_keeps_the_team_color() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);
        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(0), Some(TeamKind::Blue))]),
        )
        .unwrap();

        step(
            &mut state,
            &create_bundle(
                2,
                vec![MemberEvent::AppearanceChanged {
                    member_id: MemberId(0),
                    appearance: Appearance {
                        color: OrganismColorKind::Hot,
                        skin: SkinKind::None,
                    },
                }],
            ),
        )
        .unwrap();

        assert_eq!(
            state.members[&MemberId(0)].loadout.unwrap().appearance,
            Appearance {
                color: OrganismColorKind::Sky,
                skin: SkinKind::None,
            },
        );
    }

    #[test]
    fn step_appearance_changed_updates_a_free_for_all_color() {
        let mut state: GameState = create_state();
        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(0), None)]),
        )
        .unwrap();
        let appearance: Appearance = Appearance {
            color: OrganismColorKind::Royal,
            skin: SkinKind::Ghost,
        };

        step(
            &mut state,
            &create_bundle(
                2,
                vec![MemberEvent::AppearanceChanged {
                    member_id: MemberId(0),
                    appearance,
                }],
            ),
        )
        .unwrap();

        assert_eq!(state.members[&MemberId(0)].loadout.unwrap().appearance, appearance);
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p shared game::step`
Expected: FAIL, compilation stops; the first error is `` error[E0425]: cannot find type `MemberEvent` in this scope ``.

- [ ] **Step 4: Write the implementation**

Replace the whole of `shared/src/game/step.rs` with:

```rust
use crate::game::{GameState, InputBundle, MemberEvent, SimulationEvent, Tick};
use crate::member::{Appearance, Member, MemberId, Score};
use crate::organism::spawn;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepError {
    TickMismatch { expected: Tick, received: Tick },
    MemberIdOutOfOrder { minimum: MemberId, received: MemberId },
}

/// Advances the state by one tick; on `Err` the state is untouched.
pub fn step(state: &mut GameState, bundle: &InputBundle) -> Result<Vec<SimulationEvent>, StepError> {
    check_bundle(state, bundle)?;

    let mut simulation_events: Vec<SimulationEvent> = Vec::new();

    for member_event in &bundle.member_events {
        let simulation_event: Option<SimulationEvent> = apply_member_event(state, member_event);
        simulation_events.extend(simulation_event);
    }

    state.tick = bundle.tick;

    Ok(simulation_events)
}

fn check_bundle(state: &GameState, bundle: &InputBundle) -> Result<(), StepError> {
    let expected_tick: Tick = state.tick.next();
    if bundle.tick != expected_tick {
        return Err(StepError::TickMismatch {
            expected: expected_tick,
            received: bundle.tick,
        });
    }

    let mut next_member_id: MemberId = state.next_member_id;

    for member_event in &bundle.member_events {
        let MemberEvent::Joined { member_id, .. } = member_event else {
            continue;
        };

        if *member_id < next_member_id {
            return Err(StepError::MemberIdOutOfOrder {
                minimum: next_member_id,
                received: *member_id,
            });
        }

        next_member_id = member_id.next();
    }

    Ok(())
}

fn apply_member_event(state: &mut GameState, member_event: &MemberEvent) -> Option<SimulationEvent> {
    match member_event {
        MemberEvent::Joined {
            member_id,
            screen_name,
            role,
            loadout,
            team,
        } => {
            let member: Member = Member {
                member_id: *member_id,
                screen_name: screen_name.clone(),
                role: *role,
                loadout: loadout.map(|loadout| loadout.with_team_color(*team)),
                team: *team,
                score: Score::zero(),
                organism: None,
            };

            Some(add_member(state, member))
        }
        MemberEvent::Left { member_id } => {
            let removed_member: Option<Member> = state.members.remove(member_id);

            removed_member.map(|_| SimulationEvent::MemberLeft { member_id: *member_id })
        }
        MemberEvent::SpawnRequested {
            member_id,
            loadout,
            team,
        } => spawn::spawn_member(state, *member_id, *loadout, *team),
        MemberEvent::AppearanceChanged { member_id, appearance } => {
            change_appearance(state, *member_id, *appearance);

            None
        }
    }
}

fn add_member(state: &mut GameState, member: Member) -> SimulationEvent {
    let member_id: MemberId = member.member_id;
    state.members.insert(member_id, member);
    state.next_member_id = member_id.next();

    SimulationEvent::MemberJoined { member_id }
}

/// A member without a loadout has no appearance to change.
fn change_appearance(state: &mut GameState, member_id: MemberId, appearance: Appearance) {
    let Some(member) = state.members.get_mut(&member_id) else {
        return;
    };

    let Some(loadout) = member.loadout.as_mut() else {
        return;
    };

    loadout.appearance = appearance.with_team_color(member.team);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::member::{MemberRoleKind, OrganismColorKind, SkinKind, TeamKind};
    use crate::world::WorldShapeKind;

    fn create_bundle(tick: u32, member_events: Vec<MemberEvent>) -> InputBundle {
        InputBundle {
            tick: Tick(tick),
            member_events,
            player_inputs: Vec::new(),
        }
    }

    fn create_joined_event(member_id: MemberId, team: Option<TeamKind>) -> MemberEvent {
        MemberEvent::Joined {
            member_id,
            screen_name: format!("player {}", member_id.0),
            role: MemberRoleKind::Participant,
            loadout: Some(test_fixture::create_loadout()),
            team,
        }
    }

    fn create_state() -> GameState {
        test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800)
    }

    #[test]
    fn step_rejects_a_tick_mismatch_untouched() {
        let mut state: GameState = create_state();
        let state_before: GameState = state.clone();

        let result: Result<Vec<SimulationEvent>, StepError> = step(
            &mut state,
            &create_bundle(2, vec![create_joined_event(MemberId(0), None)]),
        );

        assert_eq!(
            result,
            Err(StepError::TickMismatch {
                expected: Tick(1),
                received: Tick(2),
            }),
        );
        assert_eq!(state, state_before);
    }

    #[test]
    fn step_rejects_a_member_id_below_the_next_untouched() {
        let mut state: GameState = create_state();
        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(3), None)]),
        )
        .unwrap();
        let state_before: GameState = state.clone();

        let result: Result<Vec<SimulationEvent>, StepError> = step(
            &mut state,
            &create_bundle(
                2,
                vec![
                    create_joined_event(MemberId(5), None),
                    create_joined_event(MemberId(5), None),
                ],
            ),
        );

        assert_eq!(
            result,
            Err(StepError::MemberIdOutOfOrder {
                minimum: MemberId(6),
                received: MemberId(5),
            }),
        );
        assert_eq!(state, state_before);
        assert_eq!(
            step(
                &mut state,
                &create_bundle(2, vec![create_joined_event(MemberId(2), None)]),
            ),
            Err(StepError::MemberIdOutOfOrder {
                minimum: MemberId(4),
                received: MemberId(2),
            }),
        );
    }

    #[test]
    fn step_advances_the_tick() {
        let mut state: GameState = create_state();

        assert_eq!(step(&mut state, &create_bundle(1, Vec::new())), Ok(Vec::new()));
        assert_eq!(state.tick, Tick(1));
    }

    #[test]
    fn step_joined_adds_a_member_without_an_organism() {
        let mut state: GameState = create_state();

        let simulation_events: Vec<SimulationEvent> = step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(4), None)]),
        )
        .unwrap();

        assert_eq!(
            simulation_events,
            vec![SimulationEvent::MemberJoined { member_id: MemberId(4) }],
        );
        assert_eq!(state.next_member_id, MemberId(5));
        assert_eq!(state.members[&MemberId(4)].screen_name, "player 4");
        assert_eq!(state.members[&MemberId(4)].organism, None);
        assert_eq!(state.members[&MemberId(4)].score, Score::zero());
    }

    #[test]
    fn step_joined_forces_the_team_color() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);

        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(0), Some(TeamKind::Red))]),
        )
        .unwrap();

        assert_eq!(
            state.members[&MemberId(0)].loadout.unwrap().appearance.color,
            OrganismColorKind::Fire,
        );
    }

    #[test]
    fn step_left_removes_the_member() {
        let mut state: GameState = create_state();
        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(0), None)]),
        )
        .unwrap();

        let simulation_events: Vec<SimulationEvent> = step(
            &mut state,
            &create_bundle(
                2,
                vec![
                    MemberEvent::Left { member_id: MemberId(0) },
                    MemberEvent::Left { member_id: MemberId(7) },
                ],
            ),
        )
        .unwrap();

        assert_eq!(
            simulation_events,
            vec![SimulationEvent::MemberLeft { member_id: MemberId(0) }],
        );
        assert!(state.members.is_empty());
        assert_eq!(state.next_member_id, MemberId(1));
    }

    #[test]
    fn step_spawn_requested_follows_its_join_in_bundle_order() {
        let mut state: GameState = create_state();

        let simulation_events: Vec<SimulationEvent> = step(
            &mut state,
            &create_bundle(
                1,
                vec![
                    create_joined_event(MemberId(0), None),
                    MemberEvent::SpawnRequested {
                        member_id: MemberId(0),
                        loadout: test_fixture::create_loadout(),
                        team: None,
                    },
                ],
            ),
        )
        .unwrap();

        let cursor: WorldPoint = test_fixture::get_organism(&state, MemberId(0)).cursor;
        assert_eq!(
            simulation_events,
            vec![
                SimulationEvent::MemberJoined { member_id: MemberId(0) },
                SimulationEvent::OrganismSpawned {
                    member_id: MemberId(0),
                    cursor,
                },
            ],
        );
    }

    #[test]
    fn step_appearance_changed_keeps_the_team_color() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);
        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(0), Some(TeamKind::Blue))]),
        )
        .unwrap();

        step(
            &mut state,
            &create_bundle(
                2,
                vec![MemberEvent::AppearanceChanged {
                    member_id: MemberId(0),
                    appearance: Appearance {
                        color: OrganismColorKind::Hot,
                        skin: SkinKind::None,
                    },
                }],
            ),
        )
        .unwrap();

        assert_eq!(
            state.members[&MemberId(0)].loadout.unwrap().appearance,
            Appearance {
                color: OrganismColorKind::Sky,
                skin: SkinKind::None,
            },
        );
    }

    #[test]
    fn step_appearance_changed_updates_a_free_for_all_color() {
        let mut state: GameState = create_state();
        step(
            &mut state,
            &create_bundle(1, vec![create_joined_event(MemberId(0), None)]),
        )
        .unwrap();
        let appearance: Appearance = Appearance {
            color: OrganismColorKind::Royal,
            skin: SkinKind::Ghost,
        };

        step(
            &mut state,
            &create_bundle(
                2,
                vec![MemberEvent::AppearanceChanged {
                    member_id: MemberId(0),
                    appearance,
                }],
            ),
        )
        .unwrap();

        assert_eq!(state.members[&MemberId(0)].loadout.unwrap().appearance, appearance);
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p shared game::step`
Expected: the `shared` library tests report `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 91 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 6: Commit**

```sh
git status
git add ./shared/src/game/input_bundle.rs ./shared/src/game/mod.rs ./shared/src/game/step.rs
git commit -m "step bundle checks and membership events"
```

`git status` before the commit lists only the paths staged above.

### Task 16: Step cursors, growth, death bookkeeping and re-tightening

**Files:**
- Modify: `shared/src/game/step.rs`
- Test: inline `mod tests` in `shared/src/game/step.rs`

Tick-order phases 2 (cursor updates), 7 (births), 8 (natural deaths), 10 (death bookkeeping) and 12 (re-tightening, then `tick = bundle.tick`). Phases 3 to 6, 9 and 11 belong to phase 3 of the roadmap.

- [ ] **Step 1: Write the failing tests**

In `shared/src/game/step.rs`, replace the imports at the top of `mod tests`

```rust
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::member::{MemberRoleKind, OrganismColorKind, SkinKind, TeamKind};
```

with

```rust
    use crate::ability::AbilityPressSet;
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, WorldPoint};
    use crate::member::{MemberRoleKind, OrganismColorKind, SkinKind, TeamKind};
    use crate::organism::CellOccupancy;
```

and append these helpers and tests at the end of `mod tests`, after a blank line and before its closing brace:

```rust
    fn create_state_with_organisms(member_ids: &[MemberId]) -> GameState {
        let mut state: GameState = create_state();

        for (index, member_id) in member_ids.iter().enumerate() {
            let x: i32 = 100 + 200 * i32::try_from(index).unwrap();
            state.members.insert(
                *member_id,
                test_fixture::create_participant_with_organism(*member_id, WorldPoint { x, y: 100 }),
            );
            state.next_member_id = member_id.next();
        }

        state
    }

    fn freeze(state: &mut GameState, member_id: MemberId) {
        test_fixture::get_organism_mut(state, member_id).abilities.frozen_until = Some(Tick(u32::MAX));
    }

    fn kill(state: &mut GameState, member_id: MemberId, last_hitter: Option<MemberId>) {
        let organism: &mut Organism = test_fixture::get_organism_mut(state, member_id);
        organism.cells = CellOccupancy::empty();
        organism.last_hitter = last_hitter;
    }

    #[test]
    fn step_moves_cursors_of_alive_organisms() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0)]);
        state.members.insert(MemberId(1), test_fixture::create_participant(MemberId(1)));
        let mut bundle: InputBundle = create_bundle(1, Vec::new());
        bundle.player_inputs = [MemberId(0), MemberId(1)]
            .iter()
            .map(|member_id| PlayerTickInput {
                member_id: *member_id,
                cursor: WorldPoint { x: 140, y: 90 },
                ability_presses: AbilityPressSet::NONE,
                aim: None,
            })
            .collect();

        step(&mut state, &bundle).unwrap();

        assert_eq!(
            test_fixture::get_organism(&state, MemberId(0)).cursor,
            WorldPoint { x: 140, y: 90 },
        );
        assert_eq!(state.members[&MemberId(1)].organism, None);
    }

    #[test]
    fn step_runs_births_then_natural_deaths() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0), MemberId(1)]);
        let mut expected_state: GameState = state.clone();
        growth::run_birth_phase(&mut expected_state, Tick(1));
        growth::run_natural_death_phase(&mut expected_state);
        for member_id in [MemberId(0), MemberId(1)] {
            test_fixture::get_organism_mut(&mut expected_state, member_id).cells.retighten();
        }

        step(&mut state, &create_bundle(1, Vec::new())).unwrap();

        assert_eq!(state.rng, expected_state.rng);
        assert_eq!(state.members, expected_state.members);
    }

    #[test]
    fn step_records_a_death_with_kill_credit() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0), MemberId(1)]);
        kill(&mut state, MemberId(0), Some(MemberId(1)));

        let simulation_events: Vec<SimulationEvent> = step(&mut state, &create_bundle(1, Vec::new())).unwrap();

        assert_eq!(
            simulation_events,
            vec![SimulationEvent::OrganismDied {
                member_id: MemberId(0),
                credited_to: Some(MemberId(1)),
            }],
        );
        assert_eq!(state.members[&MemberId(0)].organism, None);
        assert_eq!(state.members[&MemberId(0)].score.deaths, 1);
        assert_eq!(state.members[&MemberId(1)].score.kills, 1);
    }

    #[test]
    fn step_gives_no_credit_for_a_suicide() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0)]);
        kill(&mut state, MemberId(0), Some(MemberId(0)));

        let simulation_events: Vec<SimulationEvent> = step(&mut state, &create_bundle(1, Vec::new())).unwrap();

        assert_eq!(
            simulation_events,
            vec![SimulationEvent::OrganismDied {
                member_id: MemberId(0),
                credited_to: None,
            }],
        );
        assert_eq!(state.members[&MemberId(0)].score.kills, 0);
        assert_eq!(state.members[&MemberId(0)].score.deaths, 1);
    }

    #[test]
    fn step_gives_no_credit_to_a_departed_hitter() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0), MemberId(1)]);
        kill(&mut state, MemberId(0), Some(MemberId(1)));

        let simulation_events: Vec<SimulationEvent> = step(
            &mut state,
            &create_bundle(1, vec![MemberEvent::Left { member_id: MemberId(1) }]),
        )
        .unwrap();

        assert_eq!(
            simulation_events,
            vec![
                SimulationEvent::MemberLeft { member_id: MemberId(1) },
                SimulationEvent::OrganismDied {
                    member_id: MemberId(0),
                    credited_to: None,
                },
            ],
        );
    }

    #[test]
    fn step_retightens_cell_occupancies() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0)]);
        freeze(&mut state, MemberId(0));
        let organism: &mut Organism = test_fixture::get_organism_mut(&mut state, MemberId(0));
        organism.cells.insert(LatticeCoordinate { i: 5, j: 5 });
        organism.cells.remove(LatticeCoordinate { i: 5, j: 5 });

        step(&mut state, &create_bundle(1, Vec::new())).unwrap();

        assert_eq!(
            test_fixture::get_organism(&state, MemberId(0)).cells,
            CellOccupancy::with_cell(LatticeCoordinate { i: 0, j: 0 }),
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared game::step`
Expected: FAIL, compilation stops; the first error is `` error[E0425]: cannot find type `Organism` in this scope ``.

- [ ] **Step 3: Write the implementation**

In `shared/src/game/step.rs`, replace the imports at the top of the file

```rust
use crate::game::{GameState, InputBundle, MemberEvent, SimulationEvent, Tick};
use crate::member::{Appearance, Member, MemberId, Score};
use crate::organism::spawn;
```

with

```rust
use crate::game::{GameState, InputBundle, MemberEvent, PlayerTickInput, SimulationEvent, Tick};
use crate::member::{Appearance, Member, MemberId, Score};
use crate::organism::Organism;
use crate::organism::{growth, spawn};
```

In `step`, replace

```rust
    state.tick = bundle.tick;
```

with

```rust
    apply_cursor_updates(state, &bundle.player_inputs);
    growth::run_birth_phase(state, bundle.tick);
    growth::run_natural_death_phase(state);

    let death_events: Vec<SimulationEvent> = record_deaths(state);
    simulation_events.extend(death_events);

    retighten_cell_occupancies(state);
    state.tick = bundle.tick;
```

Insert directly above `#[cfg(test)]`:

```rust
/// Inputs of members without an organism are ignored.
fn apply_cursor_updates(state: &mut GameState, player_inputs: &[PlayerTickInput]) {
    for player_input in player_inputs {
        let organism: Option<&mut Organism> =
            state.members.get_mut(&player_input.member_id).and_then(|member| member.organism.as_mut());
        let Some(organism) = organism else {
            continue;
        };

        organism.cursor = player_input.cursor;
    }
}

/// Kill credit goes to the last hitter unless it is the victim or no longer a member.
fn record_deaths(state: &mut GameState) -> Vec<SimulationEvent> {
    let dead_member_ids: Vec<MemberId> = state
        .members
        .values()
        .filter(|member| member.organism.as_ref().is_some_and(|organism| organism.cells.is_empty()))
        .map(|member| member.member_id)
        .collect();
    let mut death_events: Vec<SimulationEvent> = Vec::new();

    for member_id in dead_member_ids {
        let Some(member) = state.members.get_mut(&member_id) else {
            continue;
        };

        let last_hitter: Option<MemberId> = member.organism.take().and_then(|organism| organism.last_hitter);
        member.score.deaths += 1;

        let credited_to: Option<MemberId> =
            last_hitter.filter(|hitter_id| *hitter_id != member_id && state.members.contains_key(hitter_id));
        let killer: Option<&mut Member> = credited_to.and_then(|killer_id| state.members.get_mut(&killer_id));
        if let Some(killer) = killer {
            killer.score.kills += 1;
        }

        death_events.push(SimulationEvent::OrganismDied { member_id, credited_to });
    }

    death_events
}

fn retighten_cell_occupancies(state: &mut GameState) {
    for member in state.members.values_mut() {
        let Some(organism) = member.organism.as_mut() else {
            continue;
        };

        organism.cells.retighten();
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared game::step`
Expected: the `shared` library tests report `test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 91 filtered out`.

Run: `cargo fmt -p shared -- --check`
Expected: no output, exit status 0.

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/src/game/step.rs
git commit -m "step cursors, growth, death bookkeeping and re-tightening"
```

`git status` before the commit lists only the paths staged above.

### Task 17: Growth statistics against the original rules

**Files:**
- Create: `shared/tests/growth_statistics.rs`
- Create: `scripts/test/test-growth-statistics.sh`
- Test: `shared/tests/growth_statistics.rs` (ignored integration test)

This test measures behaviour the previous tasks already implement, so it passes on its first run; it is the done criterion "the growth statistics test matches the reference means". The reference means come from a throwaway model of `Org.js` (master): single organism, no border, no opponents, float cursor, birth sites with multiplicity, reset to one cell at the cursor on death; changes per tick are births plus natural deaths.

- [ ] **Step 1: Write the statistics test**

`shared/tests/growth_statistics.rs`:

```rust
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
```

- [ ] **Step 2: Confirm it is ignored in the normal suite**

Run: `cargo test -p shared --test growth_statistics`
Expected: `test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 3: Write the script**

`scripts/test/test-growth-statistics.sh`:

```sh
#!/usr/bin/env bash
# Runs the ignored growth statistics test, which compares growth against means measured from the original rules.

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

cargo test -p shared --release --test growth_statistics -- --ignored --nocapture
```

Run: `chmod +x ./scripts/test/test-growth-statistics.sh`

- [ ] **Step 4: Run the statistics**

Run: `./scripts/test/test-growth-statistics.sh`
Expected (release build, about 2 s of test time), within the tolerances of 10% for cells and 15% for changes:

```text
Default Still: cells 64.0 (reference 64.0), changes per tick 6.22 (reference 6.2)
Default Moving: cells 42.7 (reference 42.5), changes per tick 7.78 (reference 7.7)
Compress Still: cells 37.7 (reference 37.8), changes per tick 0.79 (reference 0.8)
Compress Moving: cells 23.4 (reference 23.0), changes per tick 4.45 (reference 4.4)
Extend Still: cells 124.1 (reference 124.8), changes per tick 7.97 (reference 7.8)
Extend Moving: cells 82.1 (reference 81.2), changes per tick 10.40 (reference 10.4)
test growth_statistics_match_the_original_rules ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 5: Commit**

```sh
git status
git add ./shared/tests/growth_statistics.rs ./scripts/test/test-growth-statistics.sh
git commit -m "growth statistics test and script"
```

`git status` before the commit lists only the paths staged above.

### Task 18: Phase completion check

**Files:** none (verification only; no commit).

- [ ] **Step 1: Formatting**

Run: `cargo fmt --all -- --check`
Expected: no output, exit status 0.

- [ ] **Step 2: Build and test the workspace**

Run: `cargo build --workspace && cargo test --workspace; echo "exit=$?"`
Expected: no warnings; the `shared` library tests report `test result: ok. 106 passed; 0 failed; 0 ignored`, `forbidden_operations` reports `1 passed`, `growth_statistics` reports `0 passed; 0 failed; 1 ignored`, every other `test result:` line is `ok`, then `exit=0`.

- [ ] **Step 3: wasm32 type check**

Run: `./scripts/test/check-wasm.sh; echo "exit=$?"`
Expected: the three checks end with `Finished`, then `exit=0`.

- [ ] **Step 4: Growth statistics**

Run: `./scripts/test/test-growth-statistics.sh; echo "exit=$?"`
Expected: `test result: ok. 1 passed`, then `exit=0`.

- [ ] **Step 5: Confirm the tree**

Run: `git status --short`
Expected: no output.

Run: `git log --oneline --reverse $(git log --grep '^>>> branch: simulation-growth' --format=%h)..HEAD`
Expected, oldest first (hashes vary; review-fix commits, if any, sit after their task):

```text
<hash> phase 2 plan
<hash> phase 2 plan review
<hash> forbidden operations source scan
<hash> geometry types
<hash> Pcg32 random number generator
<hash> cell occupancy bitmap
<hash> member kinds and team colours
<hash> tick and ability model with predicates
<hash> organism with exposed and adjacent enumeration
<hash> world bounds and border rule
<hash> growth chance tables
<hash> game settings and mode codes
<hash> game state with members, score and round state
<hash> birth phase
<hash> natural death phase
<hash> spawn search and organism placement
<hash> step bundle checks and membership events
<hash> step cursors, growth, death bookkeeping and re-tightening
<hash> growth statistics test and script
```

## Roadmap coverage

- `geometry`: Task 2.
- `random/pcg32` with the published PCG32 reference vectors, a golden sequence and the even-increment rejection: Task 3.
- `world` (`contains_cell` for rectangle and ellipse, offset and shrunk worlds; shrink and restore): Task 8; the border rule applied to births and natural deaths: Tasks 12 and 13.
- `organism`: `CellOccupancy` (Task 4), exposed and adjacent enumeration with multiplicity (Task 7), growth chance tables with the golden digest (Task 9), births with collision across unaligned lattices, teammate blocking, rotation and frozen skip (Task 12), natural deaths with the range rule and frozen and immortal skips (Task 13), spawn search with hazards and attempt limit, and `place_organism` (Task 14).
- `member` data: Tasks 5 and 11. `game_settings`: Task 10. `game_state`: Task 11.
- `step` with membership, cursors, births, deaths, death bookkeeping (deaths, kill credit for suicide and departed hitters, `OrganismDied`) and re-tightening: Tasks 15 and 16.
- Forbidden-operations source scan: Task 1. Golden chance-table digest: Task 9. `Pcg32` reference vectors: Task 3.
- Done criteria: every growth, border and collision unit test (Tasks 4 to 16) and the growth statistics test (Task 17), checked together in Task 18.

## Deviations from the plan

- Test counts: the `shared` library tests report 111 passed (plan: 106) and `forbidden_operations` 3 passed (plan: 1); the extra tests are listed below.
- `shared/tests/forbidden_operations.rs`:
  - `atanh` added to the float function list, which design's `atan*` covers.
  - Patterns `f64::<name>` and `f32::<name>` without the trailing `(`, so function paths such as `f64::sin` used as values are caught too.
  - Constants renamed or added: `FORBIDDEN_FLOAT_FUNCTION_NAMES`, `BITCODE_CRATE_NAME`.
  - Two self-tests of the scanner (`find_violations_reports_forbidden_operations_outside_permitted_directories`, `find_violations_permits_exempt_directories`) and their helper `create_source_file`.
- `geometry.rs`: private helpers `widened_axis_differences` and `widened_distance_squared`, shared by both `distance_squared` functions and `is_within_cell_collision`; extra test `world_point_distance_squared_sums_squared_axis_differences`.
- `cell_occupancy.rs`: constant `BITS_PER_BYTE` in place of the literal 8; `Range` imported.
- `organism.rs`: private `empty_neighbors` iterator shared by `exposed_cells` and `adjacent_sites`.
- `world.rs`: `WorldBounds::right` and `WorldBounds::bottom` are `pub`, since the spawn range reads them.
- `growth_chance_table.rs`: `DRAW_SPACE_SIZE` derived from `ALWAYS_PASSES_THRESHOLD`; constant `CERTAIN_CHANCE_PERCENT` in place of the literal 100; the threshold doc moved from the type onto each field.
- `growth.rs`: the `CollisionIndex` doc comment dropped; `run_birth_phase_draws_once_per_site_in_order` computes each site's own distance instead of assuming 36 for every site.
- `spawn.rs`:
  - `SPAWN_MARGIN_PIXELS` derived from a new `SPAWN_BUFFER_PIXELS` and `CELL_WIDTH_PIXELS`.
  - Helpers `get_pixels_rounded_up`, `get_pixels_rounded_down` and `draw_coordinate` in place of `get_span`.
  - The hazard check split into `is_inside_spore_secretion`, `is_inside_shot_secretion` and `is_inside_toxin_field`.
  - Extra tests: the empty spawn range gives `None` without drawing (`find_spawn_position` and `spawn_member`), and spawning before a round starts (`Waiting`, `PreRound`) is allowed; the round-in-progress test also covers `PostRound`.
  - Test helpers renamed (`get_point_offset_horizontally`, `create_member_with_covering_organism`); the margin test derives its bounds from `SPAWN_MARGIN_PIXELS`.
- `game_settings.rs`: the mode codes are constants shared by `code()` and `TryFrom<&str>`; extra test `code_gives_the_three_letter_mode_codes`.
- `game_state.rs` tests: world sizes use `geometry::SUBPIXELS_PER_PIXEL`.
- `step.rs`: blank lines only.
- `growth_statistics.rs`: constant `MILLIPIXELS_PER_PIXEL` and helper `round_millipixels_to_pixels`. Measured means equal the plan's.
- Commits: each task's review fixes sit in extra commits after the task (`task <n> quality fixes`, `format game state tests`), and the final review added `game mode code constants` and `phase 2 review style fixes`.
