# Phase 3: abilities, rounds and scoring implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** the rest of the simulation in `shared`: ability activation, projectiles, damage, the ability constants and icons, survival rounds, the leaderboard, team assignment, the remaining simulation events, the state checksum and the complete tick order of `step`, proven by a 10,000-tick scripted game that runs identically twice and matches a committed golden checksum sequence.

**Architecture:** every module follows `docs/architecture/overview.md` (module tree of `shared`, module rules) and `docs/architecture/simulation.md` (data model, tick order, abilities, world, spawn, rounds, scoring, teams, events); determinism follows `docs/architecture/determinism.md`, the checksum `docs/architecture/protocol.md#checksums-and-resync`. Each tick-order phase is one function in its feature module (`activation`, `projectile`, `damage`, `round`), tested on its own and then wired into `step`. The checksum hashes the wire encoding of the state, so the sending half of the `GameStateSerialOut` wire tree is built here, ahead of the rest of the protocol in phase 4.

**Tech stack:** Rust 1.95 (edition 2024), `libm` 0.2 (square root and rounding of projectile directions), `bitcode` 0.6 (derives and `encode`, inside `shared/src/protocol/` only); no new dependencies.

---

## Ground rules for the executor

- Repository `/Users/singularity/bacter-rs`, branch `simulation-abilities`. Run `git -C /Users/singularity/bacter-rs branch --show-current` first; stop if it does not print `simulation-abilities`.
- Every command runs from the repository root `/Users/singularity/bacter-rs`.
- Never switch branches, push, rebase or amend. Stage by explicit path, run `git status` before each commit, one-line commit messages without attribution.
- Never run `cargo clippy`.
- The code below is `rustfmt`-formatted with the repository's `rustfmt.toml`; transcribe it exactly, so `cargo fmt --all -- --check` stays silent.
- Edits are given in three forms: a whole file; "replace" followed by text that occurs exactly once in the file and its replacement; "insert directly above" a unique anchor. "Append inside `mod tests`" means directly above the closing brace of the file's test module, which is the last line of the file.
- A new file's failing test is the file holding only its `#[cfg(test)] mod tests` block; the implementation step then writes the whole file.
- Builds stay warning-free; a warning is a defect to fix before committing.
- Expected test counts assume the tasks are done in order with the code exactly as given; a count that differs means a step was missed or changed.
- The plan was executed end to end in a scratch copy of this branch while it was written: every expected error, test count and checksum below is the observed output.

## Decisions taken where the design is silent or contradictory

- Checksum ahead of the protocol: the design defines `get_state_checksum` in `protocol/state_checksum.rs` as FNV-1a 64 over `protocol::encode_game_state(&GameStateSerialOut::from(&state))`, while the roadmap puts the checksum in phase 3 and the wire types in phase 4. Phase 3 builds the `GameStateSerialOut` tree (`game_state_serial.rs`, `member_serial.rs`, `organism_serial.rs`, `geometry_serial.rs`) with both bitcode derives and the sending-side `From<&Model>` conversions only, plus `protocol::encode_game_state`. Phase 4 adds the `TryFrom` conversions, every decode, the messages, `PROTOCOL_VERSION`, limits, the replay format and the snapshot round-trip test.
- Golden checksum fixture without a replay format: `shared/tests/fixtures/scripted_game.checksums`, one line per tick, `<tick> <checksum as 16 lowercase hexadecimal digits>`, written by the ignored test `regenerate_scripted_game_checksums` in `shared/tests/determinism.rs` through `scripts/test/regenerate-replay-fixtures.sh`. Phase 4 adds `scripted_game.replay`, moves the scripted players into `regenerate_scripted_game_fixture` in `shared/tests/replay.rs`, and has `protocol_dump replay --checksums` write this line format.
- Scripted game: survival (rounds, shrinking and force spawns in one game), 800 px rectangle, player minimum 4, cap 16; 16 Participants whose loadouts follow the bits of their index, one Spectator, a departure at tick 4000, a late joiner at tick 4001 and an appearance change at tick 6000; the script has its own `Pcg32`; cursors step at most 3 px per axis per tick towards random targets; each key is pressed with probability 1/32 per tick. Skirmish team rules are covered by unit tests.
- Timer expiry: a phase advances when its deadline is at or before the tick being stepped, and its cooldown counts from that tick (the same as from the deadline, since every tick is stepped). A received effect set at tick T lasts ticks T to T + duration - 1.
- Each loadout kind reports its Active and cooldown lengths (`active_ticks`, `cooldown_ticks`); a compress or freeze caster's `first` or `second` is Active for the effect length (50 or 57 ticks) and then cools for its kind's cooldown (57 or 86).
- `ShotEffectKind { Compress, Freeze }` names the two shot slots (`slot_index` 0 and 1) and is the `kind` of `SimulationEvent::EffectApplied`; the design names no type for either.
- Press order: inputs in ascending member id (bundle order between inputs of one member), presses of one input in the order first, second, third, fourth. A pop applies its effect at once, so later presses of the same tick see it.
- A shot press without an aim or with a zero aim is a no-op; so is a shot launch when an earlier press of the tick took the organism's last cell. A spore launch from an organism with no exposed cells still enters Flying, with no spores.
- Shot hits: the target's own neutralize field protects it (simulation.md, shot pop); acid and toxin are stopped by any member's neutralize field (rule 5). One `EffectApplied` per target; the caster's carried timer is set once per hit pop.
- Field and secretion radius checks are three functions in `ability_model.rs` (`is_inside_field`, `is_inside_spore_secretion`, `is_inside_shot_secretion`) with `get_neutralize_field_center` and `get_toxin_field_center`; the spawn hazard checks are rewritten onto them.
- Directions: spore offsets are taken on the lattice (the same direction as in pixels); unit vectors go through `libm::sqrt` and `libm::round`, as the chance tables use `libm`.
- Shot cell selection compares cosines exactly in i128 through dot-product signs and cross-multiplied squares, with saturating products; exact while offsets stay below 2^24.
- Force spawn (`spawn::force_spawn_participants`): every organism is discarded first, without a death; then Participants with a loadout spawn in ascending id while the alive count is below the player cap; one without a position gets `SpawnRejected { PositionNotFound }` and the next is tried. `spawn_member` now converts the member before its search, which draws the same numbers.
- Rounds: at most one transition per tick; in PreRound the checks run cancel, force spawn, start. `player_minimum` absent counts as 0 (never the case in survival). Ending a round emits `RoundPhaseChanged { PostRound }` then `RoundWon`; leaving PostRound restores the world, emits `RoundPhaseChanged { Waiting }`, then the force spawn's events.
- Countdown: `RoundState::get_countdown_seconds(tick)`, `Some` in PreRound and PostRound, from the ticks left of the 114-tick delay.
- Survival shrinking is `round::run_survival_shrink_phase`.
- Teams: `TEAM_ORDER` and `get_game_teams` live in `member.rs` beside `TeamKind`, since the leaderboard and team assignment both list a game's teams. Team sizes count Participants only. A rejected choice names the first team, in team order, smaller than the requested one (the original's message); `TeamChoiceRejectionKind::TeamNotInGame` covers a team outside the game's teams, for phase 5 to map.
- Leaderboard: `LeaderboardRow { subject: LeaderboardSubjectKind, score: Score }` and `get_leaderboard_columns(mode) -> Vec<LeaderboardColumnKind>`; column titles and row text belong to the web view (phase 8). `format_kill_death_ratio` gives the K:D text.
- Icons: the original's pen arrays as strings of L, U, R, D; `get_icon_pixels` returns offsets from the slot centre; secrete shares the spore path.
- Tick helpers: `Tick::plus`, `Tick::ticks_since`, the `const fn` `tick::get_ticks_from_milliseconds` (every duration constant is written as the original milliseconds through it) and `tick::get_seconds_rounded_up`.
- `state_checksum::get_fnv1a_64_digest` is shared with the golden chance-table digest test, which keeps its golden value.
- `GameSettings` validation stays in phase 4.

## File structure

- Modify `shared/src/game/tick.rs`: tick arithmetic, milliseconds to ticks, ticks to whole seconds.
- Modify `shared/src/ability/ability_constants.rs`: ability durations, cooldowns and projectile speeds beside the radii.
- Modify `shared/src/ability/ability_model.rs`: kind durations, `ShotEffectKind`, `AbilityPhase::is_ready`, phase expiry, field centres, field and secretion containment, `AbilityPressSet::contains` and `with`, `AimVector::is_zero`.
- Modify `shared/src/ability/mod.rs`: declares `projectile`, `activation`, `damage`, `ability_icon`.
- Create `shared/src/ability/projectile.rs`: spore launch, shot cell selection and launch, projectile flight phase, scaled directions.
- Create `shared/src/ability/activation.rs`: timer expiry phase and ability press phase, including shot pops and their effects.
- Create `shared/src/ability/damage.rs`: damage phase (spore and shot acid, toxin, neutralize protection, last hitter).
- Create `shared/src/ability/ability_icon.rs`: `AbilityIconKind`, the pen paths, `get_icon_pixels`.
- Modify `shared/src/organism/organism.rs`: `LatticeCentroid` and `Organism::centroid`.
- Modify `shared/src/organism/spawn.rs`: hazard checks through the ability containment functions, `spawn_at_found_position`, `force_spawn_participants`.
- Modify `shared/src/organism/growth_chance_table.rs`: the digest test uses the shared FNV-1a function.
- Modify `shared/src/game/simulation_event.rs`: `EffectApplied`, `RoundPhaseChanged`, `RoundWon`.
- Modify `shared/src/game/step.rs`: the complete tick order.
- Modify `shared/src/round/round.rs`: round delays, countdown, participant count, survival shrink phase, round transitions.
- Modify `shared/src/member/member.rs`: `TEAM_ORDER`, `get_game_teams`, `is_same_team`.
- Modify `shared/src/member/mod.rs`: declares `team_assignment`.
- Create `shared/src/member/team_assignment.rs`: `TeamSizes`, `TeamChoiceRejectionKind`, `check_team_choice`.
- Modify `shared/src/member/scoreboard.rs`: leaderboard rows and columns, K:D text.
- Modify `shared/src/lib.rs`: declares `protocol`.
- Create `shared/src/protocol/mod.rs`: protocol module declarations and re-exports.
- Create `shared/src/protocol/geometry_serial.rs`: `WorldPointSerialOut`, `SubpixelPointSerial`, `SubpixelVectorSerialOut`, `LatticeCoordinateSerialOut`.
- Create `shared/src/protocol/organism_serial.rs`: `OrganismSerialOut`, `CellOccupancySerialOut`, `OrganismAbilitiesSerialOut`, the ability phase and projectile wire types.
- Create `shared/src/protocol/member_serial.rs`: `MemberSerialOut`, `ScoreSerialOut`, `LoadoutSerial`, `AppearanceSerial`, the kind mirrors, `MemberRoleKindSerialOut`.
- Create `shared/src/protocol/game_state_serial.rs`: `GameStateSerialOut`, `GameSettingsSerialOut`, `GameModeKindSerial`, `WorldShapeKindSerial`, `Pcg32SerialOut`, `WorldSerialOut`, `WorldBoundsSerialOut`, `RoundStateSerialOut`, `RoundPhaseSerialOut`.
- Create `shared/src/protocol/protocol.rs`: `encode_game_state`.
- Create `shared/src/protocol/state_checksum.rs`: `get_state_checksum`, `get_fnv1a_64_digest`.
- Create `shared/tests/determinism.rs`: scripted players, the two-run test, the golden sequence test and its ignored regeneration.
- Create `shared/tests/fixtures/scripted_game.checksums`: golden checksum per tick (generated).
- Create `scripts/test/regenerate-replay-fixtures.sh`: runs the regeneration test.

---

### Task 1: Tick arithmetic and millisecond conversion

**Files:**
- Modify: `shared/src/game/tick.rs`
- Test: inline `mod tests` in `shared/src/game/tick.rs`

Every duration of the design is a tick count rounded from the original's milliseconds; the conversion is a `const fn` so the ability and round constants can be written as the rule rather than as its result.

- [ ] **Step 1: Write the failing test**

Append to the end of `shared/src/game/tick.rs` (after a blank line):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plus_adds_ticks() {
        assert_eq!(Tick(10).plus(57), Tick(67));
    }

    #[test]
    fn ticks_since_counts_forward_and_stops_at_zero() {
        assert_eq!(Tick(114).ticks_since(Tick(14)), 100);
        assert_eq!(Tick(3).ticks_since(Tick(9)), 0);
    }

    #[test]
    fn get_ticks_from_milliseconds_rounds_to_the_nearest_tick() {
        assert_eq!(get_ticks_from_milliseconds(4500), 64);
        assert_eq!(get_ticks_from_milliseconds(6000), 86);
        assert_eq!(get_ticks_from_milliseconds(35), 1);
        assert_eq!(get_ticks_from_milliseconds(34), 0);
    }

    #[test]
    fn get_seconds_rounded_up_takes_the_ceiling() {
        assert_eq!(get_seconds_rounded_up(114), 8);
        assert_eq!(get_seconds_rounded_up(100), 7);
        assert_eq!(get_seconds_rounded_up(101), 8);
        assert_eq!(get_seconds_rounded_up(0), 0);
    }
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib game::tick`
Expected: compilation fails; the first error is `` error[E0599]: no method named `plus` found for struct `tick::Tick` in the current scope ``.

- [ ] **Step 3: Implement**

Replace the whole of `shared/src/game/tick.rs` with:

```rust
pub const TICK_PERIOD_MILLISECONDS: u32 = 70;
const MILLISECONDS_PER_SECOND: u32 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick(pub u32);

impl Tick {
    pub fn next(self) -> Tick {
        Tick(self.0.wrapping_add(1))
    }

    pub fn plus(self, tick_count: u32) -> Tick {
        Tick(self.0.wrapping_add(tick_count))
    }

    /// Zero when `earlier` is not before `self`.
    pub fn ticks_since(self, earlier: Tick) -> u32 {
        self.0.saturating_sub(earlier.0)
    }
}

/// Rounded to the nearest tick, halves up.
pub const fn get_ticks_from_milliseconds(milliseconds: u32) -> u32 {
    (milliseconds + TICK_PERIOD_MILLISECONDS / 2) / TICK_PERIOD_MILLISECONDS
}

pub fn get_seconds_rounded_up(tick_count: u32) -> u32 {
    let milliseconds: u64 = u64::from(tick_count) * u64::from(TICK_PERIOD_MILLISECONDS);
    let seconds: u64 = milliseconds.div_ceil(u64::from(MILLISECONDS_PER_SECOND));

    u32::try_from(seconds).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plus_adds_ticks() {
        assert_eq!(Tick(10).plus(57), Tick(67));
    }

    #[test]
    fn ticks_since_counts_forward_and_stops_at_zero() {
        assert_eq!(Tick(114).ticks_since(Tick(14)), 100);
        assert_eq!(Tick(3).ticks_since(Tick(9)), 0);
    }

    #[test]
    fn get_ticks_from_milliseconds_rounds_to_the_nearest_tick() {
        assert_eq!(get_ticks_from_milliseconds(4500), 64);
        assert_eq!(get_ticks_from_milliseconds(6000), 86);
        assert_eq!(get_ticks_from_milliseconds(35), 1);
        assert_eq!(get_ticks_from_milliseconds(34), 0);
    }

    #[test]
    fn get_seconds_rounded_up_takes_the_ceiling() {
        assert_eq!(get_seconds_rounded_up(114), 8);
        assert_eq!(get_seconds_rounded_up(100), 7);
        assert_eq!(get_seconds_rounded_up(101), 8);
        assert_eq!(get_seconds_rounded_up(0), 0);
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib game::tick`
Expected: no warnings; `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 111 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 115 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/game/tick.rs
git status --short
git commit -m "tick arithmetic and millisecond conversion"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 2: Ability durations, cooldowns and speeds

**Files:**
- Modify: `shared/src/ability/ability_constants.rs`
- Test: inline `mod tests` in `shared/src/ability/ability_constants.rs`

- [ ] **Step 1: Write the failing test**

Append to the end of `shared/src/ability/ability_constants.rs` (after a blank line):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_match_the_designed_tick_counts() {
        assert_eq!(
            [
                EXTEND_ACTIVE_TICKS,
                EXTEND_COOLDOWN_TICKS,
                COMPRESS_EFFECT_TICKS,
                COMPRESS_COOLDOWN_TICKS,
                IMMORTALITY_ACTIVE_TICKS,
                IMMORTALITY_COOLDOWN_TICKS,
                FREEZE_EFFECT_TICKS,
                FREEZE_COOLDOWN_TICKS,
                NEUTRALIZE_ACTIVE_TICKS,
                NEUTRALIZE_COOLDOWN_TICKS,
                TOXIN_ACTIVE_TICKS,
                TOXIN_COOLDOWN_TICKS,
            ],
            [64, 57, 50, 57, 50, 86, 57, 86, 50, 93, 57, 86],
        );
        assert_eq!(
            [
                SPORE_FLIGHT_TICKS,
                SPORE_COOLDOWN_TICKS,
                SPORE_SECRETION_TICKS,
                SHOT_FLIGHT_TICKS,
                SHOT_COOLDOWN_TICKS,
                SHOT_SECRETION_TICKS,
            ],
            [24, 107, 11, 21, 29, 11],
        );
    }

    #[test]
    fn speeds_scale_the_original_packet_speeds_to_ticks() {
        assert_eq!(SPORE_SPEED_SUBPIXELS_PER_TICK, 10_752);
        assert_eq!(SHOT_SPEED_SUBPIXELS_PER_TICK, 8960);
    }
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib ability::ability_constants`
Expected: compilation fails; the first error is `` error[E0425]: cannot find value `EXTEND_ACTIVE_TICKS` in this scope ``.

- [ ] **Step 3: Implement**

Replace the whole of `shared/src/ability/ability_constants.rs` with:

```rust
use crate::game::tick;
use crate::geometry;

/// The original moved projectiles once per 40 ms server packet.
const ORIGINAL_PACKET_PERIOD_MILLISECONDS: i32 = 40;
const ORIGINAL_SPORE_PIXELS_PER_PACKET: i32 = 6;
const ORIGINAL_SHOT_PIXELS_PER_PACKET: i32 = 5;

pub const EXTEND_ACTIVE_TICKS: u32 = tick::get_ticks_from_milliseconds(4500);
pub const EXTEND_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(4000);
pub const COMPRESS_EFFECT_TICKS: u32 = tick::get_ticks_from_milliseconds(3500);
pub const COMPRESS_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(4000);
pub const IMMORTALITY_ACTIVE_TICKS: u32 = tick::get_ticks_from_milliseconds(3500);
pub const IMMORTALITY_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(6000);
pub const FREEZE_EFFECT_TICKS: u32 = tick::get_ticks_from_milliseconds(4000);
pub const FREEZE_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(6000);
pub const NEUTRALIZE_ACTIVE_TICKS: u32 = tick::get_ticks_from_milliseconds(3500);
pub const NEUTRALIZE_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(6500);
pub const TOXIN_ACTIVE_TICKS: u32 = tick::get_ticks_from_milliseconds(4000);
pub const TOXIN_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(6000);
pub const SPORE_FLIGHT_TICKS: u32 = tick::get_ticks_from_milliseconds(1700);
/// Counted from the end of the flight or of the secretion.
pub const SPORE_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(7500);
pub const SPORE_SECRETION_TICKS: u32 = tick::get_ticks_from_milliseconds(800);
pub const SHOT_FLIGHT_TICKS: u32 = tick::get_ticks_from_milliseconds(1500);
/// Per slot, counted from the end of the flight or of the secretion.
pub const SHOT_COOLDOWN_TICKS: u32 = tick::get_ticks_from_milliseconds(2000);
pub const SHOT_SECRETION_TICKS: u32 = tick::get_ticks_from_milliseconds(800);

pub const SPORE_SPEED_SUBPIXELS_PER_TICK: i32 =
    ORIGINAL_SPORE_PIXELS_PER_PACKET * geometry::SUBPIXELS_PER_PIXEL * tick::TICK_PERIOD_MILLISECONDS as i32
        / ORIGINAL_PACKET_PERIOD_MILLISECONDS;
pub const SHOT_SPEED_SUBPIXELS_PER_TICK: i32 =
    ORIGINAL_SHOT_PIXELS_PER_PACKET * geometry::SUBPIXELS_PER_PIXEL * tick::TICK_PERIOD_MILLISECONDS as i32
        / ORIGINAL_PACKET_PERIOD_MILLISECONDS;

/// Floor of `72 * 2.9²` px² in subpixel²; the comparison is `<=`.
pub const SPORE_SECRETION_RADIUS_SQUARED_SUBPIXELS: i64 = 634_933_739;
/// Floor of `151.38` px² in subpixel²; the comparison is `<=`.
pub const SHOT_SECRETION_RADIUS_SQUARED_SUBPIXELS: i64 = 158_733_434;
/// Neutralize and toxin, 60 px; the comparison is `<=`.
pub const FIELD_RADIUS_SQUARED_PIXELS: i64 = 3600;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_match_the_designed_tick_counts() {
        assert_eq!(
            [
                EXTEND_ACTIVE_TICKS,
                EXTEND_COOLDOWN_TICKS,
                COMPRESS_EFFECT_TICKS,
                COMPRESS_COOLDOWN_TICKS,
                IMMORTALITY_ACTIVE_TICKS,
                IMMORTALITY_COOLDOWN_TICKS,
                FREEZE_EFFECT_TICKS,
                FREEZE_COOLDOWN_TICKS,
                NEUTRALIZE_ACTIVE_TICKS,
                NEUTRALIZE_COOLDOWN_TICKS,
                TOXIN_ACTIVE_TICKS,
                TOXIN_COOLDOWN_TICKS,
            ],
            [64, 57, 50, 57, 50, 86, 57, 86, 50, 93, 57, 86],
        );
        assert_eq!(
            [
                SPORE_FLIGHT_TICKS,
                SPORE_COOLDOWN_TICKS,
                SPORE_SECRETION_TICKS,
                SHOT_FLIGHT_TICKS,
                SHOT_COOLDOWN_TICKS,
                SHOT_SECRETION_TICKS,
            ],
            [24, 107, 11, 21, 29, 11],
        );
    }

    #[test]
    fn speeds_scale_the_original_packet_speeds_to_ticks() {
        assert_eq!(SPORE_SPEED_SUBPIXELS_PER_TICK, 10_752);
        assert_eq!(SHOT_SPEED_SUBPIXELS_PER_TICK, 8960);
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib ability::ability_constants`
Expected: no warnings; `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 115 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 117 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/ability/ability_constants.rs
git status --short
git commit -m "ability durations, cooldowns and speeds"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 3: Ability kind durations, shot effects and press helpers

**Files:**
- Modify: `shared/src/ability/ability_model.rs`
- Test: inline `mod tests` in `shared/src/ability/ability_model.rs`

Each loadout kind knows its own Active and cooldown lengths, so timer expiry and activation never branch on the kind for a number. `ShotEffectKind` names the effect a shot slot carries (slot 0 compress, slot 1 freeze) and is reused by `SimulationEvent::EffectApplied`.

- [ ] **Step 1: Write the failing test**

In `shared/src/ability/ability_model.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn contains_and_with_combine_presses() {
        let presses: AbilityPressSet = AbilityPressSet::FIRST.with(AbilityPressSet::FOURTH);

        assert!(presses.contains(AbilityPressSet::FIRST));
        assert!(presses.contains(AbilityPressSet::FOURTH));
        assert!(!presses.contains(AbilityPressSet::SECOND));
        assert_eq!(presses.bits(), 0b1001);
    }

    #[test]
    fn active_ticks_and_cooldown_ticks_follow_each_kind() {
        assert_eq!(FirstAbilityKind::Extend.active_ticks(), 64);
        assert_eq!(FirstAbilityKind::Compress.active_ticks(), 50);
        assert_eq!(FirstAbilityKind::Compress.cooldown_ticks(), 57);
        assert_eq!(SecondAbilityKind::Immortality.active_ticks(), 50);
        assert_eq!(SecondAbilityKind::Freeze.active_ticks(), 57);
        assert_eq!(SecondAbilityKind::Freeze.cooldown_ticks(), 86);
        assert_eq!(ThirdAbilityKind::Neutralize.cooldown_ticks(), 93);
        assert_eq!(ThirdAbilityKind::Toxin.active_ticks(), 57);
    }

    #[test]
    fn slot_index_gives_compress_slot_zero_and_freeze_slot_one() {
        assert_eq!(ShotEffectKind::Compress.slot_index(), 0);
        assert_eq!(ShotEffectKind::Freeze.slot_index(), 1);
        assert_eq!(ShotEffectKind::Freeze.effect_ticks(), 57);
    }

    #[test]
    fn is_ready_holds_only_for_ready() {
        assert!(AbilityPhase::Ready.is_ready());
        assert!(!AbilityPhase::Cooling { ready_at: Tick(3) }.is_ready());
        assert!(!AbilityPhase::Active { ends_at: Tick(3) }.is_ready());
    }

    #[test]
    fn is_zero_holds_only_for_the_zero_vector() {
        assert!(AimVector { x: 0, y: 0 }.is_zero());
        assert!(!AimVector { x: 0, y: -1 }.is_zero());
    }
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib ability::ability_model`
Expected: compilation fails; the first error is `` error[E0599]: no method named `with` found for struct `ability_model::AbilityPressSet` in the current scope ``.

- [ ] **Step 3: Implement**

In `shared/src/ability/ability_model.rs`, replace:

```rust
use crate::game::Tick;
use crate::geometry::{SubpixelPoint, SubpixelVector, WorldPoint};
```

with:

```rust
use crate::ability::ability_constants;
use crate::game::Tick;
use crate::geometry::{SubpixelPoint, SubpixelVector, WorldPoint};
```

In `shared/src/ability/ability_model.rs`, replace:

```rust
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

```

with:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstAbilityKind {
    Extend,
    Compress,
}

impl FirstAbilityKind {
    /// Extend's own duration, or how long a compress caster's `first` stays Active after a hit.
    pub fn active_ticks(self) -> u32 {
        match self {
            FirstAbilityKind::Extend => ability_constants::EXTEND_ACTIVE_TICKS,
            FirstAbilityKind::Compress => ability_constants::COMPRESS_EFFECT_TICKS,
        }
    }

    pub fn cooldown_ticks(self) -> u32 {
        match self {
            FirstAbilityKind::Extend => ability_constants::EXTEND_COOLDOWN_TICKS,
            FirstAbilityKind::Compress => ability_constants::COMPRESS_COOLDOWN_TICKS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecondAbilityKind {
    Immortality,
    Freeze,
}

impl SecondAbilityKind {
    /// Immortality's own duration, or how long a freeze caster's `second` stays Active after a hit.
    pub fn active_ticks(self) -> u32 {
        match self {
            SecondAbilityKind::Immortality => ability_constants::IMMORTALITY_ACTIVE_TICKS,
            SecondAbilityKind::Freeze => ability_constants::FREEZE_EFFECT_TICKS,
        }
    }

    pub fn cooldown_ticks(self) -> u32 {
        match self {
            SecondAbilityKind::Immortality => ability_constants::IMMORTALITY_COOLDOWN_TICKS,
            SecondAbilityKind::Freeze => ability_constants::FREEZE_COOLDOWN_TICKS,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThirdAbilityKind {
    Neutralize,
    Toxin,
}

impl ThirdAbilityKind {
    pub fn active_ticks(self) -> u32 {
        match self {
            ThirdAbilityKind::Neutralize => ability_constants::NEUTRALIZE_ACTIVE_TICKS,
            ThirdAbilityKind::Toxin => ability_constants::TOXIN_ACTIVE_TICKS,
        }
    }

    pub fn cooldown_ticks(self) -> u32 {
        match self {
            ThirdAbilityKind::Neutralize => ability_constants::NEUTRALIZE_COOLDOWN_TICKS,
            ThirdAbilityKind::Toxin => ability_constants::TOXIN_COOLDOWN_TICKS,
        }
    }
}

/// The effect a shot carries; each has its own shot slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotEffectKind {
    Compress,
    Freeze,
}

impl ShotEffectKind {
    pub fn slot_index(self) -> usize {
        match self {
            ShotEffectKind::Compress => 0,
            ShotEffectKind::Freeze => 1,
        }
    }

    pub fn effect_ticks(self) -> u32 {
        match self {
            ShotEffectKind::Compress => ability_constants::COMPRESS_EFFECT_TICKS,
            ShotEffectKind::Freeze => ability_constants::FREEZE_EFFECT_TICKS,
        }
    }
}

```

In `shared/src/ability/ability_model.rs`, replace:

```rust
    pub fn is_active(self) -> bool {
        matches!(self, AbilityPhase::Active { .. })
    }
}
```

with:

```rust
    pub fn is_active(self) -> bool {
        matches!(self, AbilityPhase::Active { .. })
    }

    pub fn is_ready(self) -> bool {
        self == AbilityPhase::Ready
    }
}
```

In `shared/src/ability/ability_model.rs`, replace:

```rust
    pub fn bits(self) -> u8 {
        self.bits
    }
}
```

with:

```rust
    pub fn bits(self) -> u8 {
        self.bits
    }

    pub fn contains(self, press: AbilityPressSet) -> bool {
        self.bits & press.bits == press.bits
    }

    pub fn with(self, press: AbilityPressSet) -> AbilityPressSet {
        AbilityPressSet {
            bits: self.bits | press.bits,
        }
    }
}
```

In `shared/src/ability/ability_model.rs`, replace:

```rust
pub struct AimVector {
    pub x: i16,
    pub y: i16,
}
```

with:

```rust
pub struct AimVector {
    pub x: i16,
    pub y: i16,
}

impl AimVector {
    pub fn is_zero(self) -> bool {
        self.x == 0 && self.y == 0
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib ability::ability_model`
Expected: no warnings; `test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 111 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 122 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/ability/ability_model.rs
git status --short
git commit -m "ability kind durations, shot effects and press helpers"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 4: Ability phase expiry

**Files:**
- Modify: `shared/src/ability/ability_model.rs`
- Test: inline `mod tests` in `shared/src/ability/ability_model.rs`

Phase 3 of the tick order advances every timer whose tick has arrived. Each phase type owns its transition; a cooldown runs from the tick the Active, Flying or Secreting phase ended.

- [ ] **Step 1: Write the failing test**

In `shared/src/ability/ability_model.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn ability_phase_expire_cools_then_readies_on_the_deadline() {
        let mut phase: AbilityPhase = AbilityPhase::Active { ends_at: Tick(64) };

        phase.expire(Tick(63), 57);
        assert_eq!(phase, AbilityPhase::Active { ends_at: Tick(64) });

        phase.expire(Tick(64), 57);
        assert_eq!(phase, AbilityPhase::Cooling { ready_at: Tick(121) });

        phase.expire(Tick(120), 57);
        assert_eq!(phase, AbilityPhase::Cooling { ready_at: Tick(121) });

        phase.expire(Tick(121), 57);
        assert_eq!(phase, AbilityPhase::Ready);
    }

    #[test]
    fn spore_phase_expire_drops_the_spores_and_cools() {
        let spore: Projectile = Projectile {
            position: SubpixelPoint { x: 0, y: 0 },
            velocity: SubpixelVector { x: 1, y: 0 },
        };
        let mut flying_phase: SporePhase = SporePhase::Flying {
            ends_at: Tick(24),
            spores: vec![spore],
        };
        let mut secreting_phase: SporePhase = SporePhase::Secreting {
            ends_at: Tick(30),
            spores: vec![spore],
        };

        flying_phase.expire(Tick(24));
        secreting_phase.expire(Tick(30));

        assert_eq!(flying_phase, SporePhase::Cooling { ready_at: Tick(131) });
        assert_eq!(secreting_phase, SporePhase::Cooling { ready_at: Tick(137) });

        flying_phase.expire(Tick(131));
        assert_eq!(flying_phase, SporePhase::Ready);
    }

    #[test]
    fn shot_phase_expire_cools_a_flight_or_a_secretion() {
        let mut flying_phase: ShotPhase = ShotPhase::Flying {
            ends_at: Tick(21),
            shot: Projectile {
                position: SubpixelPoint { x: 0, y: 0 },
                velocity: SubpixelVector { x: 0, y: 1 },
            },
        };
        let mut secreting_phase: ShotPhase = ShotPhase::Secreting {
            ends_at: Tick(11),
            center: SubpixelPoint { x: 0, y: 0 },
        };

        flying_phase.expire(Tick(20));
        assert!(matches!(flying_phase, ShotPhase::Flying { .. }));

        flying_phase.expire(Tick(21));
        secreting_phase.expire(Tick(11));

        assert_eq!(flying_phase, ShotPhase::Cooling { ready_at: Tick(50) });
        assert_eq!(secreting_phase, ShotPhase::Cooling { ready_at: Tick(40) });

        secreting_phase.expire(Tick(40));
        assert_eq!(secreting_phase, ShotPhase::Ready);
    }
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib ability::ability_model`
Expected: compilation fails; the first error is `` error[E0599]: no method named `expire` found for enum `ability_model::AbilityPhase` in the current scope ``.

- [ ] **Step 3: Implement**

In `shared/src/ability/ability_model.rs`, replace:

```rust
    pub fn is_ready(self) -> bool {
        self == AbilityPhase::Ready
    }
}
```

with:

```rust
    pub fn is_ready(self) -> bool {
        self == AbilityPhase::Ready
    }

    /// An ended Active phase cools for `cooldown_ticks` from `tick`; an ended Cooling phase becomes Ready.
    pub fn expire(&mut self, tick: Tick, cooldown_ticks: u32) {
        let next_phase: Option<AbilityPhase> = match *self {
            AbilityPhase::Active { ends_at } if ends_at <= tick => Some(AbilityPhase::Cooling {
                ready_at: tick.plus(cooldown_ticks),
            }),
            AbilityPhase::Cooling { ready_at } if ready_at <= tick => Some(AbilityPhase::Ready),
            AbilityPhase::Ready | AbilityPhase::Active { .. } | AbilityPhase::Cooling { .. } => None,
        };

        if let Some(next_phase) = next_phase {
            *self = next_phase;
        }
    }
}
```

In `shared/src/ability/ability_model.rs`, insert directly above `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum ShotPhase {`:

```rust
impl SporePhase {
    /// An ended flight or secretion drops its spores and cools; an ended Cooling phase becomes Ready.
    pub fn expire(&mut self, tick: Tick) {
        let next_phase: Option<SporePhase> = match self {
            SporePhase::Flying { ends_at, .. } | SporePhase::Secreting { ends_at, .. } if *ends_at <= tick => {
                Some(SporePhase::Cooling {
                    ready_at: tick.plus(ability_constants::SPORE_COOLDOWN_TICKS),
                })
            }
            SporePhase::Cooling { ready_at } if *ready_at <= tick => Some(SporePhase::Ready),
            SporePhase::Ready
            | SporePhase::Flying { .. }
            | SporePhase::Secreting { .. }
            | SporePhase::Cooling { .. } => None,
        };

        if let Some(next_phase) = next_phase {
            *self = next_phase;
        }
    }
}

```

In `shared/src/ability/ability_model.rs`, insert directly above `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct Projectile {`:

```rust
impl ShotPhase {
    /// An ended flight or secretion cools; an ended Cooling phase becomes Ready.
    pub fn expire(&mut self, tick: Tick) {
        let next_phase: Option<ShotPhase> = match *self {
            ShotPhase::Flying { ends_at, .. } | ShotPhase::Secreting { ends_at, .. } if ends_at <= tick => {
                Some(ShotPhase::Cooling {
                    ready_at: tick.plus(ability_constants::SHOT_COOLDOWN_TICKS),
                })
            }
            ShotPhase::Cooling { ready_at } if ready_at <= tick => Some(ShotPhase::Ready),
            ShotPhase::Ready | ShotPhase::Flying { .. } | ShotPhase::Secreting { .. } | ShotPhase::Cooling { .. } => {
                None
            }
        };

        if let Some(next_phase) = next_phase {
            *self = next_phase;
        }
    }
}

```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib ability::ability_model`
Expected: no warnings; `test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 111 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 125 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/ability/ability_model.rs
git status --short
git commit -m "ability phase expiry"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 5: Field and secretion containment

**Files:**
- Modify: `shared/src/ability/ability_model.rs`
- Modify: `shared/src/organism/spawn.rs`
- Test: inline `mod tests` in `shared/src/ability/ability_model.rs`; the existing spawn hazard tests in `shared/src/organism/spawn.rs`

The radius rules of the neutralize and toxin fields and of both secretions live in one place: spawn hazards, shot hits and the damage phase all call these functions (the bug-fix ledger's single neutralize check). The spawn search is rewritten onto them; its existing hazard tests must keep passing.

- [ ] **Step 1: Write the failing test**

In `shared/src/ability/ability_model.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn get_field_centers_follow_the_third_ability_kind() {
        let mut abilities: OrganismAbilities = create_active_abilities();
        abilities.third_center = Some(WorldPoint { x: 5, y: 6 });
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

        assert_eq!(
            abilities.get_neutralize_field_center(&neutralize_loadout),
            Some(WorldPoint { x: 5, y: 6 }),
        );
        assert_eq!(abilities.get_toxin_field_center(&neutralize_loadout), None);
        assert_eq!(
            abilities.get_toxin_field_center(&toxin_loadout),
            Some(WorldPoint { x: 5, y: 6 }),
        );

        abilities.third = AbilityPhase::Cooling { ready_at: Tick(90) };

        assert_eq!(abilities.get_toxin_field_center(&toxin_loadout), None);
    }

    #[test]
    fn is_inside_field_includes_the_radius() {
        let field_center: WorldPoint = WorldPoint { x: 100, y: 100 };

        assert!(is_inside_field(field_center, WorldPoint { x: 160, y: 100 }));
        assert!(!is_inside_field(field_center, WorldPoint { x: 161, y: 100 }));
    }

    #[test]
    fn is_inside_secretion_includes_the_radius() {
        let center: SubpixelPoint = SubpixelPoint { x: 0, y: 0 };

        assert!(is_inside_spore_secretion(center, SubpixelPoint { x: 25_197, y: 0 }));
        assert!(!is_inside_spore_secretion(center, SubpixelPoint { x: 25_198, y: 0 }));
        assert!(is_inside_shot_secretion(center, SubpixelPoint { x: 12_598, y: 0 }));
        assert!(!is_inside_shot_secretion(center, SubpixelPoint { x: 12_599, y: 0 }));
    }
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib ability::ability_model`
Expected: compilation fails; the first error is `` error[E0425]: cannot find function `is_inside_spore_secretion` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/ability/ability_model.rs`, insert directly above `pub fn is_compressed(&self) -> bool {`:

```rust
    pub fn get_neutralize_field_center(&self, loadout: &Loadout) -> Option<WorldPoint> {
        self.third_center.filter(|_| self.is_neutralize_field_active(loadout))
    }

    pub fn get_toxin_field_center(&self, loadout: &Loadout) -> Option<WorldPoint> {
        self.third_center.filter(|_| self.is_toxin_field_active(loadout))
    }

```

In `shared/src/ability/ability_model.rs`, insert directly above `#[cfg(test)] mod tests {`:

```rust
pub fn is_inside_field(field_center: WorldPoint, point: WorldPoint) -> bool {
    field_center.distance_squared(point) <= ability_constants::FIELD_RADIUS_SQUARED_PIXELS
}

pub fn is_inside_spore_secretion(spore_position: SubpixelPoint, point: SubpixelPoint) -> bool {
    spore_position.distance_squared(point) <= ability_constants::SPORE_SECRETION_RADIUS_SQUARED_SUBPIXELS
}

pub fn is_inside_shot_secretion(shot_center: SubpixelPoint, point: SubpixelPoint) -> bool {
    shot_center.distance_squared(point) <= ability_constants::SHOT_SECRETION_RADIUS_SQUARED_SUBPIXELS
}

```

In `shared/src/organism/spawn.rs`, replace:

```rust
use crate::ability::ability_constants;
```

with:

```rust
use crate::ability;
```

In `shared/src/organism/spawn.rs`, replace:

```rust
fn is_inside_spore_secretion(organism: &Organism, candidate: WorldPoint) -> bool {
    let candidate_subpixels: SubpixelPoint = candidate.to_subpixel_point();

    match &organism.abilities.spore {
        SporePhase::Secreting { spores, .. } => spores.iter().any(|spore| {
            spore.position.distance_squared(candidate_subpixels)
                <= ability_constants::SPORE_SECRETION_RADIUS_SQUARED_SUBPIXELS
        }),
        SporePhase::Ready | SporePhase::Flying { .. } | SporePhase::Cooling { .. } => false,
    }
}

fn is_inside_shot_secretion(organism: &Organism, candidate: WorldPoint) -> bool {
    let candidate_subpixels: SubpixelPoint = candidate.to_subpixel_point();

    organism.abilities.shots.iter().any(|shot| match shot {
        ShotPhase::Secreting { center, .. } => {
            center.distance_squared(candidate_subpixels) <= ability_constants::SHOT_SECRETION_RADIUS_SQUARED_SUBPIXELS
        }
        ShotPhase::Ready | ShotPhase::Flying { .. } | ShotPhase::Cooling { .. } => false,
    })
}

fn is_inside_toxin_field(member: &Member, organism: &Organism, candidate: WorldPoint) -> bool {
    let is_toxin_field_active: bool =
        member.loadout.as_ref().is_some_and(|loadout| organism.abilities.is_toxin_field_active(loadout));

    is_toxin_field_active
        && organism.abilities.third_center.is_some_and(|third_center| {
            third_center.distance_squared(candidate) <= ability_constants::FIELD_RADIUS_SQUARED_PIXELS
        })
}

```

with:

```rust
fn is_inside_spore_secretion(organism: &Organism, candidate: WorldPoint) -> bool {
    let candidate_subpixels: SubpixelPoint = candidate.to_subpixel_point();

    match &organism.abilities.spore {
        SporePhase::Secreting { spores, .. } => {
            spores.iter().any(|spore| ability::is_inside_spore_secretion(spore.position, candidate_subpixels))
        }
        SporePhase::Ready | SporePhase::Flying { .. } | SporePhase::Cooling { .. } => false,
    }
}

fn is_inside_shot_secretion(organism: &Organism, candidate: WorldPoint) -> bool {
    let candidate_subpixels: SubpixelPoint = candidate.to_subpixel_point();

    organism.abilities.shots.iter().any(|shot| match shot {
        ShotPhase::Secreting { center, .. } => ability::is_inside_shot_secretion(*center, candidate_subpixels),
        ShotPhase::Ready | ShotPhase::Flying { .. } | ShotPhase::Cooling { .. } => false,
    })
}

fn is_inside_toxin_field(member: &Member, organism: &Organism, candidate: WorldPoint) -> bool {
    let toxin_field_center: Option<WorldPoint> =
        member.loadout.as_ref().and_then(|loadout| organism.abilities.get_toxin_field_center(loadout));

    toxin_field_center.is_some_and(|field_center| ability::is_inside_field(field_center, candidate))
}

```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib -- ability::ability_model organism::spawn`
Expected: no warnings; `test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 93 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 128 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/ability/ability_model.rs shared/src/organism/spawn.rs
git status --short
git commit -m "field and secretion containment"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 6: Organism centroid

**Files:**
- Modify: `shared/src/organism/organism.rs`
- Test: inline `mod tests` in `shared/src/organism/organism.rs`

Spore and shot launch measure each cell's direction from the centroid. The centroid stays an exact integer pair (lattice sum and count); offsets are scaled by the count, which leaves every direction and every cosine unchanged.

- [ ] **Step 1: Write the failing test**

In `shared/src/organism/organism.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn centroid_sums_the_lattice_coordinates() {
        let organism: Organism = create_organism(&[(0, 0), (3, 1), (-1, 2)]);

        assert_eq!(
            organism.centroid(),
            LatticeCentroid {
                sum_i: 2,
                sum_j: 3,
                cell_count: 3,
            },
        );
    }

    #[test]
    fn get_scaled_offset_points_away_from_the_centroid() {
        let organism: Organism = create_organism(&[(0, 0), (2, 0), (1, 3)]);
        let centroid: LatticeCentroid = organism.centroid();

        assert_eq!(centroid.get_scaled_offset(LatticeCoordinate { i: 2, j: 0 }), (3, -3));
        assert_eq!(centroid.get_scaled_offset(LatticeCoordinate { i: 1, j: 1 }), (0, 0));
    }

    #[test]
    fn without_removes_one_cell_from_the_centroid() {
        let organism: Organism = create_organism(&[(0, 0), (2, 0), (1, 3)]);
        let centroid: LatticeCentroid = organism.centroid().without(LatticeCoordinate { i: 1, j: 3 });

        assert_eq!(
            centroid,
            LatticeCentroid {
                sum_i: 2,
                sum_j: 0,
                cell_count: 2,
            },
        );
        assert_eq!(centroid.get_scaled_offset(LatticeCoordinate { i: 2, j: 0 }), (2, 0));
    }
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib organism::organism`
Expected: compilation fails; the first error is `` error[E0422]: cannot find struct, variant or union type `LatticeCentroid` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/organism/organism.rs`, insert directly above `#[derive(Clone, Debug, PartialEq, Eq)] pub struct Organism {`:

```rust
/// The centroid of a cell set as the exact lattice sum over the cell count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LatticeCentroid {
    pub sum_i: i64,
    pub sum_j: i64,
    pub cell_count: i64,
}

impl LatticeCentroid {
    /// `(cell - centroid) * cell_count`: the direction from the centroid, scaled to stay an integer.
    pub fn get_scaled_offset(self, lattice_coordinate: LatticeCoordinate) -> (i64, i64) {
        (
            i64::from(lattice_coordinate.i) * self.cell_count - self.sum_i,
            i64::from(lattice_coordinate.j) * self.cell_count - self.sum_j,
        )
    }

    pub fn without(self, lattice_coordinate: LatticeCoordinate) -> LatticeCentroid {
        LatticeCentroid {
            sum_i: self.sum_i - i64::from(lattice_coordinate.i),
            sum_j: self.sum_j - i64::from(lattice_coordinate.j),
            cell_count: self.cell_count - 1,
        }
    }
}

```

In `shared/src/organism/organism.rs`, insert directly above `/// Cells with fewer than four orthogonal own neighbours, in lattice order.`:

```rust
    pub fn centroid(&self) -> LatticeCentroid {
        let mut centroid: LatticeCentroid = LatticeCentroid {
            sum_i: 0,
            sum_j: 0,
            cell_count: 0,
        };

        for lattice_coordinate in self.cells.iter() {
            centroid.sum_i += i64::from(lattice_coordinate.i);
            centroid.sum_j += i64::from(lattice_coordinate.j);
            centroid.cell_count += 1;
        }

        centroid
    }

```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib organism::organism`
Expected: no warnings; `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 124 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 131 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/organism/organism.rs
git status --short
git commit -m "organism centroid"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 7: Spore launch

**Files:**
- Create: `shared/src/ability/projectile.rs`
- Modify: `shared/src/ability/mod.rs`
- Test: inline `mod tests` in `shared/src/ability/projectile.rs`

Spores are taken one at a time in lattice order, each aimed away from the centroid of the cells still in the organism (the original's progressive centroid). Directions are unit vectors from integer offsets through `libm::sqrt`, scaled by the speed in subpixels and rounded once (determinism rules).

- [ ] **Step 1: Write the failing test**

Create `shared/src/ability/projectile.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{SubpixelPoint, WorldPoint};
    use crate::organism::CellOccupancy;

    fn create_organism(lattice_coordinates: &[(i32, i32)]) -> Organism {
        let mut organism: Organism = Organism::new(WorldPoint { x: 100, y: 100 });
        organism.cells = CellOccupancy::empty();

        for (i, j) in lattice_coordinates {
            organism.cells.insert(LatticeCoordinate { i: *i, j: *j });
        }

        organism
    }

    fn get_cells(organism: &Organism) -> Vec<LatticeCoordinate> {
        organism.cells.iter().collect()
    }

    #[test]
    fn get_scaled_direction_keeps_axis_directions_exact() {
        assert_eq!(get_scaled_direction(5, 0, 10_752), SubpixelVector { x: 10_752, y: 0 },);
        assert_eq!(get_scaled_direction(0, -1, 8960), SubpixelVector { x: 0, y: -8960 },);
    }

    #[test]
    fn get_scaled_direction_rounds_each_component_once() {
        assert_eq!(get_scaled_direction(1, 1, 10_752), SubpixelVector { x: 7603, y: 7603 },);
        assert_eq!(get_scaled_direction(-3, 4, 8960), SubpixelVector { x: -5376, y: 7168 },);
    }

    #[test]
    fn launch_spores_uses_the_progressive_centroid() {
        let mut organism: Organism = create_organism(&[(0, 0), (1, 0), (2, 0)]);

        let spores: Vec<Projectile> = launch_spores(&mut organism);

        assert!(organism.cells.is_empty());
        assert_eq!(
            spores,
            vec![
                Projectile {
                    position: SubpixelPoint { x: 102_400, y: 102_400 },
                    velocity: SubpixelVector { x: -10_752, y: 0 },
                },
                Projectile {
                    position: SubpixelPoint { x: 108_544, y: 102_400 },
                    velocity: SubpixelVector { x: -10_752, y: 0 },
                },
            ],
        );
    }

    #[test]
    fn launch_spores_keeps_enclosed_cells() {
        let block: Vec<(i32, i32)> = (-1..=1).flat_map(|j| (-1..=1).map(move |i| (i, j))).collect();
        let mut organism: Organism = create_organism(&block);

        let spores: Vec<Projectile> = launch_spores(&mut organism);

        assert_eq!(get_cells(&organism), vec![LatticeCoordinate { i: 0, j: 0 }]);
        assert_eq!(spores.len(), 8);
        assert_eq!(spores[0].velocity, SubpixelVector { x: -7603, y: -7603 });
        assert_eq!(spores[1].velocity, SubpixelVector { x: -1187, y: -10_686 });
    }

    #[test]
    fn launch_spores_drops_a_spore_at_the_centroid() {
        let mut organism: Organism = create_organism(&[(0, 0)]);

        let spores: Vec<Projectile> = launch_spores(&mut organism);

        assert_eq!(spores, Vec::new());
        assert!(organism.cells.is_empty());
    }
}
```

Replace the whole of `shared/src/ability/mod.rs` with:

```rust
pub mod ability_constants;
pub mod ability_model;
pub mod projectile;

pub use ability_constants::*;
pub use ability_model::*;
pub use projectile::*;
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib ability::projectile`
Expected: compilation fails; the first error is `` error[E0425]: cannot find type `Organism` in this scope ``.

- [ ] **Step 3: Implement**

Replace the whole of `shared/src/ability/projectile.rs` with:

```rust
use crate::ability::Projectile;
use crate::ability::ability_constants;
use crate::geometry::{LatticeCoordinate, SubpixelVector};
use crate::organism::{LatticeCentroid, Organism};

/// Every exposed cell leaves the organism in lattice order; each flies away from the centroid of the cells still
/// in the organism, and one with no offset from it is dropped without a projectile.
pub fn launch_spores(organism: &mut Organism) -> Vec<Projectile> {
    let exposed_cells: Vec<LatticeCoordinate> = organism.exposed_cells();
    let mut centroid: LatticeCentroid = organism.centroid();
    let mut spores: Vec<Projectile> = Vec::new();

    for lattice_coordinate in exposed_cells {
        let (offset_i, offset_j): (i64, i64) = centroid.get_scaled_offset(lattice_coordinate);
        centroid = centroid.without(lattice_coordinate);
        organism.cells.remove(lattice_coordinate);

        if offset_i == 0 && offset_j == 0 {
            continue;
        }

        spores.push(Projectile {
            position: organism.cell_center(lattice_coordinate).to_subpixel_point(),
            velocity: get_scaled_direction(offset_i, offset_j, ability_constants::SPORE_SPEED_SUBPIXELS_PER_TICK),
        });
    }

    spores
}

/// `speed` along the non-zero `(x, y)`, each component rounded once.
pub fn get_scaled_direction(x: i64, y: i64, speed: i32) -> SubpixelVector {
    // Exact: offsets and aims are far below 2^53.
    let x_float: f64 = x as f64;
    let y_float: f64 = y as f64;
    let length: f64 = libm::sqrt(x_float * x_float + y_float * y_float);
    let speed_float: f64 = f64::from(speed);

    // Bounded by `speed`.
    SubpixelVector {
        x: libm::round(x_float / length * speed_float) as i32,
        y: libm::round(y_float / length * speed_float) as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{SubpixelPoint, WorldPoint};
    use crate::organism::CellOccupancy;

    fn create_organism(lattice_coordinates: &[(i32, i32)]) -> Organism {
        let mut organism: Organism = Organism::new(WorldPoint { x: 100, y: 100 });
        organism.cells = CellOccupancy::empty();

        for (i, j) in lattice_coordinates {
            organism.cells.insert(LatticeCoordinate { i: *i, j: *j });
        }

        organism
    }

    fn get_cells(organism: &Organism) -> Vec<LatticeCoordinate> {
        organism.cells.iter().collect()
    }

    #[test]
    fn get_scaled_direction_keeps_axis_directions_exact() {
        assert_eq!(get_scaled_direction(5, 0, 10_752), SubpixelVector { x: 10_752, y: 0 },);
        assert_eq!(get_scaled_direction(0, -1, 8960), SubpixelVector { x: 0, y: -8960 },);
    }

    #[test]
    fn get_scaled_direction_rounds_each_component_once() {
        assert_eq!(get_scaled_direction(1, 1, 10_752), SubpixelVector { x: 7603, y: 7603 },);
        assert_eq!(get_scaled_direction(-3, 4, 8960), SubpixelVector { x: -5376, y: 7168 },);
    }

    #[test]
    fn launch_spores_uses_the_progressive_centroid() {
        let mut organism: Organism = create_organism(&[(0, 0), (1, 0), (2, 0)]);

        let spores: Vec<Projectile> = launch_spores(&mut organism);

        assert!(organism.cells.is_empty());
        assert_eq!(
            spores,
            vec![
                Projectile {
                    position: SubpixelPoint { x: 102_400, y: 102_400 },
                    velocity: SubpixelVector { x: -10_752, y: 0 },
                },
                Projectile {
                    position: SubpixelPoint { x: 108_544, y: 102_400 },
                    velocity: SubpixelVector { x: -10_752, y: 0 },
                },
            ],
        );
    }

    #[test]
    fn launch_spores_keeps_enclosed_cells() {
        let block: Vec<(i32, i32)> = (-1..=1).flat_map(|j| (-1..=1).map(move |i| (i, j))).collect();
        let mut organism: Organism = create_organism(&block);

        let spores: Vec<Projectile> = launch_spores(&mut organism);

        assert_eq!(get_cells(&organism), vec![LatticeCoordinate { i: 0, j: 0 }]);
        assert_eq!(spores.len(), 8);
        assert_eq!(spores[0].velocity, SubpixelVector { x: -7603, y: -7603 });
        assert_eq!(spores[1].velocity, SubpixelVector { x: -1187, y: -10_686 });
    }

    #[test]
    fn launch_spores_drops_a_spore_at_the_centroid() {
        let mut organism: Organism = create_organism(&[(0, 0)]);

        let spores: Vec<Projectile> = launch_spores(&mut organism);

        assert_eq!(spores, Vec::new());
        assert!(organism.cells.is_empty());
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib ability::projectile`
Expected: no warnings; `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 131 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 136 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/ability/projectile.rs shared/src/ability/mod.rs
git status --short
git commit -m "spore launch"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 8: Shot cell selection and launch

**Files:**
- Modify: `shared/src/ability/projectile.rs`
- Test: inline `mod tests` in `shared/src/ability/projectile.rs`

The shot leaves from the exposed cell whose offset from the centroid has the largest cosine with the aim, compared exactly through dot-product signs and cross-multiplied squares in i128 (fixes the original's angle wraparound), and flies along the aim.

- [ ] **Step 1: Write the failing test**

In `shared/src/ability/projectile.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn select_shot_cell_maximises_the_cosine_with_the_aim() {
        let organism: Organism = create_organism(&[(0, 0), (1, 0), (2, 0), (1, 1), (1, -1)]);

        assert_eq!(
            select_shot_cell(&organism, AimVector { x: 10, y: 1 }),
            Some(LatticeCoordinate { i: 2, j: 0 }),
        );
        assert_eq!(
            select_shot_cell(&organism, AimVector { x: 1, y: -10 }),
            Some(LatticeCoordinate { i: 1, j: -1 }),
        );
    }

    #[test]
    fn select_shot_cell_handles_aims_across_the_negative_x_axis() {
        let organism: Organism = create_organism(&[(0, 0), (1, 0), (-2, -1), (-2, 1)]);

        assert_eq!(
            select_shot_cell(&organism, AimVector { x: -20, y: 1 }),
            Some(LatticeCoordinate { i: -2, j: 1 }),
        );
        assert_eq!(
            select_shot_cell(&organism, AimVector { x: -20, y: -1 }),
            Some(LatticeCoordinate { i: -2, j: -1 }),
        );
    }

    #[test]
    fn select_shot_cell_breaks_ties_in_lattice_order() {
        let organism: Organism = create_organism(&[(0, -1), (0, 0), (0, 1)]);

        assert_eq!(
            select_shot_cell(&organism, AimVector { x: 1, y: 0 }),
            Some(LatticeCoordinate { i: 0, j: -1 }),
        );
    }

    #[test]
    fn select_shot_cell_takes_a_centroid_cell_only_when_alone() {
        let single_cell_organism: Organism = create_organism(&[(4, 4)]);
        let centered_organism: Organism = create_organism(&[(0, 0), (1, 0), (2, 0)]);

        assert_eq!(
            select_shot_cell(&single_cell_organism, AimVector { x: 0, y: 1 }),
            Some(LatticeCoordinate { i: 4, j: 4 }),
        );
        assert_eq!(
            select_shot_cell(&centered_organism, AimVector { x: 0, y: 1 }),
            Some(LatticeCoordinate { i: 0, j: 0 }),
        );
    }

    #[test]
    fn launch_shot_flies_along_the_aim_from_the_chosen_cell() {
        let mut organism: Organism = create_organism(&[(0, 0), (1, 0), (2, 0)]);

        let shot: Option<Projectile> = launch_shot(&mut organism, AimVector { x: 0, y: 7 });

        assert_eq!(
            shot,
            Some(Projectile {
                position: SubpixelPoint { x: 102_400, y: 102_400 },
                velocity: SubpixelVector { x: 0, y: 8960 },
            }),
        );
        assert_eq!(
            get_cells(&organism),
            vec![LatticeCoordinate { i: 1, j: 0 }, LatticeCoordinate { i: 2, j: 0 }],
        );
    }

    #[test]
    fn launch_shot_ignores_a_zero_aim() {
        let mut organism: Organism = create_organism(&[(0, 0), (1, 0)]);

        assert_eq!(launch_shot(&mut organism, AimVector { x: 0, y: 0 }), None);
        assert_eq!(organism.cells.count(), 2);
    }

    #[test]
    fn launch_shot_can_take_the_last_cell() {
        let mut organism: Organism = create_organism(&[(0, 0)]);

        assert!(launch_shot(&mut organism, AimVector { x: 3, y: 0 }).is_some());
        assert!(organism.cells.is_empty());
    }
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib ability::projectile`
Expected: compilation fails; the first error is `` error[E0422]: cannot find struct, variant or union type `AimVector` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/ability/projectile.rs`, replace:

```rust
use crate::ability::Projectile;
use crate::ability::ability_constants;
```

with:

```rust
use std::cmp::Ordering;

use crate::ability::ability_constants;
use crate::ability::{AimVector, Projectile};
```

In `shared/src/ability/projectile.rs`, insert directly above `/// `speed` along the non-zero`:

```rust
/// `None` for a zero aim or an organism without cells; the chosen cell leaves the organism and flies along `aim`.
pub fn launch_shot(organism: &mut Organism, aim: AimVector) -> Option<Projectile> {
    if aim.is_zero() {
        return None;
    }

    let lattice_coordinate: LatticeCoordinate = select_shot_cell(organism, aim)?;
    organism.cells.remove(lattice_coordinate);

    Some(Projectile {
        position: organism.cell_center(lattice_coordinate).to_subpixel_point(),
        velocity: get_scaled_direction(
            i64::from(aim.x),
            i64::from(aim.y),
            ability_constants::SHOT_SPEED_SUBPIXELS_PER_TICK,
        ),
    })
}

/// The exposed cell whose offset from the centroid has the largest cosine with `aim`, the first in lattice order
/// on a tie; a cell at the centroid is chosen only when it is the only exposed cell.
pub fn select_shot_cell(organism: &Organism, aim: AimVector) -> Option<LatticeCoordinate> {
    let centroid: LatticeCentroid = organism.centroid();
    let exposed_cells: Vec<LatticeCoordinate> = organism.exposed_cells();
    let offset_cells: Vec<(LatticeCoordinate, (i64, i64))> = exposed_cells
        .iter()
        .map(|lattice_coordinate| (*lattice_coordinate, centroid.get_scaled_offset(*lattice_coordinate)))
        .filter(|(_, offset)| *offset != (0, 0))
        .collect();

    let Some(first_offset_cell) = offset_cells.first() else {
        return exposed_cells.first().copied();
    };

    let mut best_offset_cell: (LatticeCoordinate, (i64, i64)) = *first_offset_cell;

    for offset_cell in &offset_cells[1..] {
        let alignment_ordering: Ordering = compare_alignment(offset_cell.1, best_offset_cell.1, aim);

        if alignment_ordering == Ordering::Greater {
            best_offset_cell = *offset_cell;
        }
    }

    Some(best_offset_cell.0)
}

```

In `shared/src/ability/projectile.rs`, insert directly above `#[cfg(test)] mod tests {`:

```rust
/// Orders the cosines of `first` and `second` with `aim`, exactly.
fn compare_alignment(first: (i64, i64), second: (i64, i64), aim: AimVector) -> Ordering {
    let first_dot: i128 = get_dot_product(first, aim);
    let second_dot: i128 = get_dot_product(second, aim);
    let sign_ordering: Ordering = first_dot.signum().cmp(&second_dot.signum());

    if sign_ordering != Ordering::Equal {
        return sign_ordering;
    }

    // Exact while offsets stay below 2^24; saturates beyond.
    let first_scaled_square: i128 = first_dot.saturating_mul(first_dot).saturating_mul(get_length_squared(second));
    let second_scaled_square: i128 = second_dot.saturating_mul(second_dot).saturating_mul(get_length_squared(first));
    let magnitude_ordering: Ordering = first_scaled_square.cmp(&second_scaled_square);

    if first_dot < 0 {
        return magnitude_ordering.reverse();
    }

    magnitude_ordering
}

fn get_dot_product(offset: (i64, i64), aim: AimVector) -> i128 {
    i128::from(offset.0) * i128::from(aim.x) + i128::from(offset.1) * i128::from(aim.y)
}

fn get_length_squared(offset: (i64, i64)) -> i128 {
    i128::from(offset.0) * i128::from(offset.0) + i128::from(offset.1) * i128::from(offset.1)
}

```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib ability::projectile`
Expected: no warnings; `test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 131 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 143 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/ability/projectile.rs
git status --short
git commit -m "shot cell selection and launch"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 9: Projectile flight

**Files:**
- Modify: `shared/src/ability/projectile.rs`
- Test: inline `mod tests` in `shared/src/ability/projectile.rs`

Phase 5 of the tick order moves every Flying spore and shot by its velocity once per tick.

- [ ] **Step 1: Write the failing test**

In `shared/src/ability/projectile.rs`, replace:

```rust
    use super::*;
    use crate::geometry::{SubpixelPoint, WorldPoint};
    use crate::organism::CellOccupancy;
```

with:

```rust
    use super::*;
    use crate::ability::OrganismAbilities;
    use crate::game::{GameModeKind, Tick, test_fixture};
    use crate::geometry::{SubpixelPoint, WorldPoint};
    use crate::member::{Member, MemberId};
    use crate::organism::CellOccupancy;
    use crate::world::WorldShapeKind;
```

In `shared/src/ability/projectile.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn advance_projectile_covers_speed_times_flight_ticks() {
        let mut spore: Projectile = Projectile {
            position: SubpixelPoint { x: 0, y: 0 },
            velocity: get_scaled_direction(1, 0, ability_constants::SPORE_SPEED_SUBPIXELS_PER_TICK),
        };
        let mut shot: Projectile = Projectile {
            position: SubpixelPoint { x: 0, y: 0 },
            velocity: get_scaled_direction(0, 1, ability_constants::SHOT_SPEED_SUBPIXELS_PER_TICK),
        };

        for _ in 0..ability_constants::SPORE_FLIGHT_TICKS {
            advance_projectile(&mut spore);
        }

        for _ in 0..ability_constants::SHOT_FLIGHT_TICKS {
            advance_projectile(&mut shot);
        }

        assert_eq!(spore.position, SubpixelPoint { x: 24 * 10_752, y: 0 });
        assert_eq!(shot.position, SubpixelPoint { x: 0, y: 21 * 8960 });
    }

    #[test]
    fn run_flight_phase_moves_only_flying_projectiles() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        let mut member: Member =
            test_fixture::create_participant_with_organism(MemberId(0), WorldPoint { x: 100, y: 100 });
        let projectile: Projectile = Projectile {
            position: SubpixelPoint { x: 10, y: 20 },
            velocity: SubpixelVector { x: 3, y: -4 },
        };
        let abilities: &mut OrganismAbilities = &mut member.organism.as_mut().unwrap().abilities;
        abilities.spore = SporePhase::Flying {
            ends_at: Tick(24),
            spores: vec![projectile],
        };
        abilities.shots = [
            ShotPhase::Flying {
                ends_at: Tick(21),
                shot: projectile,
            },
            ShotPhase::Secreting {
                ends_at: Tick(11),
                center: projectile.position,
            },
        ];
        state.members.insert(MemberId(0), member);

        run_flight_phase(&mut state);

        let moved: Projectile = Projectile {
            position: SubpixelPoint { x: 13, y: 16 },
            velocity: projectile.velocity,
        };
        let abilities: &OrganismAbilities = &test_fixture::get_organism(&state, MemberId(0)).abilities;
        assert_eq!(
            abilities.spore,
            SporePhase::Flying {
                ends_at: Tick(24),
                spores: vec![moved],
            },
        );
        assert_eq!(
            abilities.shots,
            [
                ShotPhase::Flying {
                    ends_at: Tick(21),
                    shot: moved,
                },
                ShotPhase::Secreting {
                    ends_at: Tick(11),
                    center: projectile.position,
                },
            ],
        );
    }
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib ability::projectile`
Expected: compilation fails; the first error is `` error[E0425]: cannot find type `GameState` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/ability/projectile.rs`, replace:

```rust
use crate::ability::{AimVector, Projectile};
use crate::geometry::{LatticeCoordinate, SubpixelVector};
```

with:

```rust
use crate::ability::{AimVector, Projectile, ShotPhase, SporePhase};
use crate::game::GameState;
use crate::geometry::{LatticeCoordinate, SubpixelVector};
```

In `shared/src/ability/projectile.rs`, insert directly above `/// `speed` along the non-zero`:

```rust
pub fn run_flight_phase(state: &mut GameState) {
    for member in state.members.values_mut() {
        let Some(organism) = member.organism.as_mut() else {
            continue;
        };

        if let SporePhase::Flying { spores, .. } = &mut organism.abilities.spore {
            spores.iter_mut().for_each(advance_projectile);
        }

        for shot_phase in &mut organism.abilities.shots {
            if let ShotPhase::Flying { shot, .. } = shot_phase {
                advance_projectile(shot);
            }
        }
    }
}

pub fn advance_projectile(projectile: &mut Projectile) {
    projectile.position.x += projectile.velocity.x;
    projectile.position.y += projectile.velocity.y;
}

```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib ability::projectile`
Expected: no warnings; `test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 131 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 145 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/ability/projectile.rs
git status --short
git commit -m "projectile flight"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 10: Timer expiry phase

**Files:**
- Create: `shared/src/ability/activation.rs`
- Modify: `shared/src/ability/mod.rs`
- Test: inline `mod tests` in `shared/src/ability/activation.rs`

Phase 3 of the tick order: every ability phase, the field centre of a third ability that is no longer Active, and the received compress and freeze deadlines advance at the tick being stepped.

- [ ] **Step 1: Write the failing test**

Create `shared/src/ability/activation.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{AbilityPhase, FirstAbilityKind};
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::member::MemberId;
    use crate::world::WorldShapeKind;

    const CASTER_ID: MemberId = MemberId(0);
    const TARGET_ID: MemberId = MemberId(1);

    fn create_state_with_organisms(mode: GameModeKind, positions: &[WorldPoint]) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);

        for (index, position) in positions.iter().enumerate() {
            let member_id: MemberId = MemberId(u32::try_from(index).unwrap());
            state.members.insert(
                member_id,
                test_fixture::create_participant_with_organism(member_id, *position),
            );
        }

        state
    }

    fn get_abilities(state: &GameState, member_id: MemberId) -> &OrganismAbilities {
        &test_fixture::get_organism(state, member_id).abilities
    }

    #[test]
    fn expire_timers_uses_the_cooldown_of_the_chosen_kind() {
        let mut loadout: Loadout = test_fixture::create_loadout();
        loadout.first = FirstAbilityKind::Compress;
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.first = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.second = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third_center = Some(WorldPoint { x: 4, y: 4 });

        expire_timers(&mut abilities, &loadout, Tick(50));

        assert_eq!(abilities.first, AbilityPhase::Cooling { ready_at: Tick(107) });
        assert_eq!(abilities.second, AbilityPhase::Cooling { ready_at: Tick(136) });
        assert_eq!(abilities.third, AbilityPhase::Cooling { ready_at: Tick(143) });
        assert_eq!(abilities.third_center, None);
    }

    #[test]
    fn expire_timers_clears_received_effects_on_their_deadline() {
        let loadout: Loadout = test_fixture::create_loadout();
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.compressed_until = Some(Tick(60));
        abilities.frozen_until = Some(Tick(61));

        expire_timers(&mut abilities, &loadout, Tick(60));

        assert_eq!(abilities.compressed_until, None);
        assert_eq!(abilities.frozen_until, Some(Tick(61)));
    }

    #[test]
    fn expire_timers_keeps_an_active_field_centre() {
        let loadout: Loadout = test_fixture::create_loadout();
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.third = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third_center = Some(WorldPoint { x: 4, y: 4 });

        expire_timers(&mut abilities, &loadout, Tick(49));

        assert_eq!(abilities.third_center, Some(WorldPoint { x: 4, y: 4 }));
    }

    #[test]
    fn run_timer_expiry_phase_advances_every_organism() {
        let mut state: GameState = create_state_with_organisms(
            GameModeKind::FreeForAll,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 100 }],
        );

        for member_id in [CASTER_ID, TARGET_ID] {
            test_fixture::get_organism_mut(&mut state, member_id).abilities.second =
                AbilityPhase::Active { ends_at: Tick(5) };
        }

        run_timer_expiry_phase(&mut state, Tick(5));

        for member_id in [CASTER_ID, TARGET_ID] {
            assert_eq!(
                get_abilities(&state, member_id).second,
                AbilityPhase::Cooling { ready_at: Tick(91) },
            );
        }
    }
}
```

Replace the whole of `shared/src/ability/mod.rs` with:

```rust
pub mod ability_constants;
pub mod ability_model;
pub mod activation;
pub mod projectile;

pub use ability_constants::*;
pub use ability_model::*;
pub use activation::*;
pub use projectile::*;
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib ability::activation`
Expected: compilation fails; the first error is `` error[E0425]: cannot find type `GameState` in this scope ``.

- [ ] **Step 3: Implement**

Replace the whole of `shared/src/ability/activation.rs` with:

```rust
use crate::ability::{Loadout, OrganismAbilities};
use crate::game::{GameState, Tick};

pub fn run_timer_expiry_phase(state: &mut GameState, tick: Tick) {
    for member in state.members.values_mut() {
        let Some(loadout) = member.loadout else {
            continue;
        };

        let Some(organism) = member.organism.as_mut() else {
            continue;
        };

        expire_timers(&mut organism.abilities, &loadout, tick);
    }
}

pub fn expire_timers(abilities: &mut OrganismAbilities, loadout: &Loadout, tick: Tick) {
    abilities.first.expire(tick, loadout.first.cooldown_ticks());
    abilities.second.expire(tick, loadout.second.cooldown_ticks());
    abilities.third.expire(tick, loadout.third.cooldown_ticks());

    if !abilities.third.is_active() {
        abilities.third_center = None;
    }

    abilities.spore.expire(tick);

    for shot_phase in &mut abilities.shots {
        shot_phase.expire(tick);
    }

    abilities.compressed_until = abilities.compressed_until.filter(|compressed_until| *compressed_until > tick);
    abilities.frozen_until = abilities.frozen_until.filter(|frozen_until| *frozen_until > tick);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{AbilityPhase, FirstAbilityKind};
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::member::MemberId;
    use crate::world::WorldShapeKind;

    const CASTER_ID: MemberId = MemberId(0);
    const TARGET_ID: MemberId = MemberId(1);

    fn create_state_with_organisms(mode: GameModeKind, positions: &[WorldPoint]) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);

        for (index, position) in positions.iter().enumerate() {
            let member_id: MemberId = MemberId(u32::try_from(index).unwrap());
            state.members.insert(
                member_id,
                test_fixture::create_participant_with_organism(member_id, *position),
            );
        }

        state
    }

    fn get_abilities(state: &GameState, member_id: MemberId) -> &OrganismAbilities {
        &test_fixture::get_organism(state, member_id).abilities
    }

    #[test]
    fn expire_timers_uses_the_cooldown_of_the_chosen_kind() {
        let mut loadout: Loadout = test_fixture::create_loadout();
        loadout.first = FirstAbilityKind::Compress;
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.first = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.second = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third_center = Some(WorldPoint { x: 4, y: 4 });

        expire_timers(&mut abilities, &loadout, Tick(50));

        assert_eq!(abilities.first, AbilityPhase::Cooling { ready_at: Tick(107) });
        assert_eq!(abilities.second, AbilityPhase::Cooling { ready_at: Tick(136) });
        assert_eq!(abilities.third, AbilityPhase::Cooling { ready_at: Tick(143) });
        assert_eq!(abilities.third_center, None);
    }

    #[test]
    fn expire_timers_clears_received_effects_on_their_deadline() {
        let loadout: Loadout = test_fixture::create_loadout();
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.compressed_until = Some(Tick(60));
        abilities.frozen_until = Some(Tick(61));

        expire_timers(&mut abilities, &loadout, Tick(60));

        assert_eq!(abilities.compressed_until, None);
        assert_eq!(abilities.frozen_until, Some(Tick(61)));
    }

    #[test]
    fn expire_timers_keeps_an_active_field_centre() {
        let loadout: Loadout = test_fixture::create_loadout();
        let mut abilities: OrganismAbilities = OrganismAbilities::all_ready();
        abilities.third = AbilityPhase::Active { ends_at: Tick(50) };
        abilities.third_center = Some(WorldPoint { x: 4, y: 4 });

        expire_timers(&mut abilities, &loadout, Tick(49));

        assert_eq!(abilities.third_center, Some(WorldPoint { x: 4, y: 4 }));
    }

    #[test]
    fn run_timer_expiry_phase_advances_every_organism() {
        let mut state: GameState = create_state_with_organisms(
            GameModeKind::FreeForAll,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 100 }],
        );

        for member_id in [CASTER_ID, TARGET_ID] {
            test_fixture::get_organism_mut(&mut state, member_id).abilities.second =
                AbilityPhase::Active { ends_at: Tick(5) };
        }

        run_timer_expiry_phase(&mut state, Tick(5));

        for member_id in [CASTER_ID, TARGET_ID] {
            assert_eq!(
                get_abilities(&state, member_id).second,
                AbilityPhase::Cooling { ready_at: Tick(91) },
            );
        }
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib ability::activation`
Expected: no warnings; `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 145 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 149 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/ability/activation.rs shared/src/ability/mod.rs
git status --short
git commit -m "timer expiry phase"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 11: Self-cast ability presses

**Files:**
- Modify: `shared/src/ability/activation.rs`
- Test: inline `mod tests` in `shared/src/ability/activation.rs`

Phase 4 of the tick order for the abilities that act on the presser: extend, immortality, the neutralize and toxin fields (centred on the cursor, rule 6), and spore launch and secretion. Inputs apply in ascending member id, presses in the order first, second, third, fourth. The compress and freeze arms stay empty until Task 12 gives them their shot slots.

- [ ] **Step 1: Write the failing test**

In `shared/src/ability/activation.rs`, replace:

```rust
    use super::*;
    use crate::ability::{AbilityPhase, FirstAbilityKind};
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::member::MemberId;
    use crate::world::WorldShapeKind;
```

with:

```rust
    use super::*;
    use crate::ability::{AimVector, ThirdAbilityKind};
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, WorldPoint};
    use crate::world::WorldShapeKind;
```

In `shared/src/ability/activation.rs`, replace:

```rust
    const TARGET_ID: MemberId = MemberId(1);
```

with:

```rust
    const TARGET_ID: MemberId = MemberId(1);
    const PRESS_TICK: Tick = Tick(10);
```

In `shared/src/ability/activation.rs`, insert directly above `#[test] fn expire_timers_uses_the_cooldown_of_the_chosen_kind`:

```rust
    fn get_loadout_mut(state: &mut GameState, member_id: MemberId) -> &mut Loadout {
        state.members.get_mut(&member_id).unwrap().loadout.as_mut().unwrap()
    }

    fn create_input(member_id: MemberId, ability_presses: AbilityPressSet, aim: Option<AimVector>) -> PlayerTickInput {
        PlayerTickInput {
            member_id,
            cursor: WorldPoint { x: 0, y: 0 },
            ability_presses,
            aim,
        }
    }

    fn press(
        state: &mut GameState,
        member_id: MemberId,
        ability_presses: AbilityPressSet,
        aim: Option<AimVector>,
        tick: Tick,
    ) -> Vec<SimulationEvent> {
        run_ability_press_phase(state, &[create_input(member_id, ability_presses, aim)], tick)
    }

    fn insert_cells(state: &mut GameState, member_id: MemberId, lattice_coordinates: &[(i32, i32)]) {
        let organism: &mut Organism = test_fixture::get_organism_mut(state, member_id);

        for (i, j) in lattice_coordinates {
            organism.cells.insert(LatticeCoordinate { i: *i, j: *j });
        }
    }

```

In `shared/src/ability/activation.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn press_first_extends_for_its_duration_once() {
        let mut state: GameState =
            create_state_with_organisms(GameModeKind::FreeForAll, &[WorldPoint { x: 100, y: 100 }]);

        press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);
        press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, Tick(11));

        assert_eq!(
            get_abilities(&state, CASTER_ID).first,
            AbilityPhase::Active { ends_at: Tick(74) },
        );
    }

    #[test]
    fn press_second_makes_immortal() {
        let mut state: GameState =
            create_state_with_organisms(GameModeKind::FreeForAll, &[WorldPoint { x: 100, y: 100 }]);

        press(&mut state, CASTER_ID, AbilityPressSet::SECOND, None, PRESS_TICK);

        assert_eq!(
            get_abilities(&state, CASTER_ID).second,
            AbilityPhase::Active { ends_at: Tick(60) },
        );
    }

    #[test]
    fn press_third_centres_the_field_on_the_cursor() {
        let mut state: GameState =
            create_state_with_organisms(GameModeKind::FreeForAll, &[WorldPoint { x: 100, y: 100 }]);
        get_loadout_mut(&mut state, CASTER_ID).third = ThirdAbilityKind::Toxin;
        test_fixture::get_organism_mut(&mut state, CASTER_ID).cursor = WorldPoint { x: 120, y: 90 };

        press(&mut state, CASTER_ID, AbilityPressSet::THIRD, None, PRESS_TICK);

        let abilities: &OrganismAbilities = get_abilities(&state, CASTER_ID);
        assert_eq!(abilities.third, AbilityPhase::Active { ends_at: Tick(67) });
        assert_eq!(abilities.third_center, Some(WorldPoint { x: 120, y: 90 }));
    }

    #[test]
    fn press_fourth_launches_then_secretes_the_spores() {
        let mut state: GameState =
            create_state_with_organisms(GameModeKind::FreeForAll, &[WorldPoint { x: 100, y: 100 }]);
        insert_cells(&mut state, CASTER_ID, &[(1, 0)]);
        let mut expected_organism: Organism = test_fixture::get_organism(&state, CASTER_ID).clone();
        let expected_spores: Vec<Projectile> = projectile::launch_spores(&mut expected_organism);

        press(&mut state, CASTER_ID, AbilityPressSet::FOURTH, None, PRESS_TICK);

        assert_eq!(expected_spores.len(), 1);
        assert_eq!(
            test_fixture::get_organism(&state, CASTER_ID).cells,
            expected_organism.cells
        );
        assert_eq!(
            get_abilities(&state, CASTER_ID).spore,
            SporePhase::Flying {
                ends_at: Tick(34),
                spores: expected_spores.clone(),
            },
        );

        press(&mut state, CASTER_ID, AbilityPressSet::FOURTH, None, Tick(12));

        assert_eq!(
            get_abilities(&state, CASTER_ID).spore,
            SporePhase::Secreting {
                ends_at: Tick(23),
                spores: expected_spores,
            },
        );
    }

    #[test]
    fn run_ability_press_phase_ignores_members_without_an_organism() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.members.insert(CASTER_ID, test_fixture::create_participant(CASTER_ID));
        let state_before: GameState = state.clone();

        press(
            &mut state,
            CASTER_ID,
            AbilityPressSet::FIRST.with(AbilityPressSet::FOURTH),
            None,
            PRESS_TICK,
        );

        assert_eq!(state, state_before);
    }
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib ability::activation`
Expected: compilation fails; the first error is `` error[E0425]: cannot find type `MemberId` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/ability/activation.rs`, replace:

```rust
use crate::ability::{Loadout, OrganismAbilities};
use crate::game::{GameState, Tick};

```

with:

```rust
use crate::ability::ability_constants;
use crate::ability::projectile;
use crate::ability::{
    AbilityPhase, AbilityPressSet, FirstAbilityKind, Loadout, OrganismAbilities, Projectile, SecondAbilityKind,
    SporePhase,
};
use crate::game::{GameState, PlayerTickInput, SimulationEvent, Tick};
use crate::member::{Member, MemberId};
use crate::organism::Organism;

```

In `shared/src/ability/activation.rs`, insert directly above `#[cfg(test)] mod tests {`:

```rust
/// Inputs apply in ascending member id; members without an organism press nothing.
pub fn run_ability_press_phase(
    state: &mut GameState,
    player_inputs: &[PlayerTickInput],
    tick: Tick,
) -> Vec<SimulationEvent> {
    let mut ordered_player_inputs: Vec<&PlayerTickInput> = player_inputs.iter().collect();
    ordered_player_inputs.sort_by_key(|player_input| player_input.member_id);

    let mut simulation_events: Vec<SimulationEvent> = Vec::new();

    for player_input in ordered_player_inputs {
        let press_events: Vec<SimulationEvent> = apply_presses(state, player_input, tick);
        simulation_events.extend(press_events);
    }

    simulation_events
}

fn apply_presses(state: &mut GameState, player_input: &PlayerTickInput, tick: Tick) -> Vec<SimulationEvent> {
    let presses: AbilityPressSet = player_input.ability_presses;
    let mut simulation_events: Vec<SimulationEvent> = Vec::new();

    if presses.contains(AbilityPressSet::FIRST) {
        let first_press_events: Vec<SimulationEvent> = press_first(state, player_input, tick);
        simulation_events.extend(first_press_events);
    }

    if presses.contains(AbilityPressSet::SECOND) {
        let second_press_events: Vec<SimulationEvent> = press_second(state, player_input, tick);
        simulation_events.extend(second_press_events);
    }

    if presses.contains(AbilityPressSet::THIRD) {
        press_third(state, player_input.member_id, tick);
    }

    if presses.contains(AbilityPressSet::FOURTH) {
        press_fourth(state, player_input.member_id, tick);
    }

    simulation_events
}

fn press_first(state: &mut GameState, player_input: &PlayerTickInput, tick: Tick) -> Vec<SimulationEvent> {
    let Some((loadout, organism)) = get_loadout_and_organism(state, player_input.member_id) else {
        return Vec::new();
    };

    if !organism.abilities.first.is_ready() {
        return Vec::new();
    }

    match loadout.first {
        FirstAbilityKind::Extend => {
            organism.abilities.first = AbilityPhase::Active {
                ends_at: tick.plus(loadout.first.active_ticks()),
            };

            Vec::new()
        }
        FirstAbilityKind::Compress => Vec::new(),
    }
}

fn press_second(state: &mut GameState, player_input: &PlayerTickInput, tick: Tick) -> Vec<SimulationEvent> {
    let Some((loadout, organism)) = get_loadout_and_organism(state, player_input.member_id) else {
        return Vec::new();
    };

    if !organism.abilities.second.is_ready() {
        return Vec::new();
    }

    match loadout.second {
        SecondAbilityKind::Immortality => {
            organism.abilities.second = AbilityPhase::Active {
                ends_at: tick.plus(loadout.second.active_ticks()),
            };

            Vec::new()
        }
        SecondAbilityKind::Freeze => Vec::new(),
    }
}

fn press_third(state: &mut GameState, member_id: MemberId, tick: Tick) {
    let Some((loadout, organism)) = get_loadout_and_organism(state, member_id) else {
        return;
    };

    if !organism.abilities.third.is_ready() {
        return;
    }

    organism.abilities.third = AbilityPhase::Active {
        ends_at: tick.plus(loadout.third.active_ticks()),
    };
    organism.abilities.third_center = Some(organism.cursor);
}

fn press_fourth(state: &mut GameState, member_id: MemberId, tick: Tick) {
    let Some((_, organism)) = get_loadout_and_organism(state, member_id) else {
        return;
    };

    match organism.abilities.spore {
        SporePhase::Ready => {
            let spores: Vec<Projectile> = projectile::launch_spores(organism);

            organism.abilities.spore = SporePhase::Flying {
                ends_at: tick.plus(ability_constants::SPORE_FLIGHT_TICKS),
                spores,
            };
        }
        SporePhase::Flying { ref mut spores, .. } => {
            let secreting_spores: Vec<Projectile> = std::mem::take(spores);

            organism.abilities.spore = SporePhase::Secreting {
                ends_at: tick.plus(ability_constants::SPORE_SECRETION_TICKS),
                spores: secreting_spores,
            };
        }
        SporePhase::Secreting { .. } | SporePhase::Cooling { .. } => {}
    }
}

fn get_loadout_and_organism(state: &mut GameState, member_id: MemberId) -> Option<(Loadout, &mut Organism)> {
    let member: &mut Member = state.members.get_mut(&member_id)?;
    let loadout: Loadout = member.loadout?;
    let organism: &mut Organism = member.organism.as_mut()?;

    Some((loadout, organism))
}

```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib ability::activation`
Expected: no warnings; `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 145 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 154 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/ability/activation.rs
git status --short
git commit -m "self-cast ability presses"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 12: Shot slots and their effects

**Files:**
- Modify: `shared/src/ability/activation.rs`
- Modify: `shared/src/game/simulation_event.rs`
- Modify: `shared/src/member/member.rs`
- Test: inline `mod tests` in `shared/src/ability/activation.rs` and `shared/src/member/member.rs`

Compress and freeze press their shot slot: Ready launches along a non-zero aim, Flying pops into a secretion. A pop applies the carried effect once to every other non-teammate whose cell lies inside the shot secretion and outside its own neutralize field, emits `EffectApplied` per target, and only on a hit starts the caster's carried ability timer; a miss only cools the slot (faithful).

- [ ] **Step 1: Write the failing test**

In `shared/src/ability/activation.rs`, replace:

```rust
    use super::*;
    use crate::ability::{AimVector, ThirdAbilityKind};
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, WorldPoint};
    use crate::world::WorldShapeKind;
```

with:

```rust
    use super::*;
    use crate::ability::{AimVector, ThirdAbilityKind};
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, SubpixelVector};
    use crate::world::WorldShapeKind;
```

In `shared/src/ability/activation.rs`, replace:

```rust
    const TARGET_ID: MemberId = MemberId(1);
```

with:

```rust
    const TARGET_ID: MemberId = MemberId(1);
    const OTHER_TARGET_ID: MemberId = MemberId(2);
```

In `shared/src/ability/activation.rs`, insert directly above `#[test] fn expire_timers_uses_the_cooldown_of_the_chosen_kind`:

```rust
    fn set_flying_shot(state: &mut GameState, member_id: MemberId, effect: ShotEffectKind, position: WorldPoint) {
        test_fixture::get_organism_mut(state, member_id).abilities.shots[effect.slot_index()] = ShotPhase::Flying {
            ends_at: Tick(30),
            shot: Projectile {
                position: position.to_subpixel_point(),
                velocity: SubpixelVector { x: 0, y: 0 },
            },
        };
    }

```

In `shared/src/ability/activation.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn run_ability_press_phase_applies_inputs_in_ascending_member_id() {
        let mut state: GameState = create_state_with_organisms(
            GameModeKind::FreeForAll,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 130, y: 100 }],
        );
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        get_loadout_mut(&mut state, TARGET_ID).first = FirstAbilityKind::Compress;
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 130, y: 100 },
        );
        set_flying_shot(
            &mut state,
            TARGET_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 100, y: 100 },
        );

        let simulation_events: Vec<SimulationEvent> = run_ability_press_phase(
            &mut state,
            &[
                create_input(TARGET_ID, AbilityPressSet::FIRST, None),
                create_input(CASTER_ID, AbilityPressSet::FIRST, None),
            ],
            PRESS_TICK,
        );

        assert_eq!(
            simulation_events,
            vec![
                SimulationEvent::EffectApplied {
                    target: TARGET_ID,
                    caster: CASTER_ID,
                    kind: ShotEffectKind::Compress,
                },
                SimulationEvent::EffectApplied {
                    target: CASTER_ID,
                    caster: TARGET_ID,
                    kind: ShotEffectKind::Compress,
                },
            ],
        );
    }

    #[test]
    fn press_shot_slot_launches_along_a_non_zero_aim() {
        let mut state: GameState =
            create_state_with_organisms(GameModeKind::FreeForAll, &[WorldPoint { x: 100, y: 100 }]);
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        insert_cells(&mut state, CASTER_ID, &[(1, 0)]);

        press(
            &mut state,
            CASTER_ID,
            AbilityPressSet::FIRST,
            Some(AimVector { x: 0, y: 0 }),
            PRESS_TICK,
        );
        press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert_eq!(get_abilities(&state, CASTER_ID).shots[0], ShotPhase::Ready);

        press(
            &mut state,
            CASTER_ID,
            AbilityPressSet::FIRST,
            Some(AimVector { x: 5, y: 0 }),
            PRESS_TICK,
        );

        let abilities: &OrganismAbilities = get_abilities(&state, CASTER_ID);
        assert_eq!(
            abilities.shots[0],
            ShotPhase::Flying {
                ends_at: Tick(31),
                shot: Projectile {
                    position: WorldPoint { x: 106, y: 100 }.to_subpixel_point(),
                    velocity: SubpixelVector { x: 8960, y: 0 },
                },
            },
        );
        assert_eq!(abilities.first, AbilityPhase::Ready);
    }

    #[test]
    fn press_shot_slot_pops_and_compresses_each_target_once() {
        let mut state: GameState = create_state_with_organisms(
            GameModeKind::FreeForAll,
            &[
                WorldPoint { x: 100, y: 100 },
                WorldPoint { x: 300, y: 300 },
                WorldPoint { x: 308, y: 300 },
            ],
        );
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        insert_cells(&mut state, TARGET_ID, &[(1, 0)]);
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 304, y: 300 },
        );

        let simulation_events: Vec<SimulationEvent> =
            press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert_eq!(
            simulation_events,
            vec![
                SimulationEvent::EffectApplied {
                    target: TARGET_ID,
                    caster: CASTER_ID,
                    kind: ShotEffectKind::Compress,
                },
                SimulationEvent::EffectApplied {
                    target: OTHER_TARGET_ID,
                    caster: CASTER_ID,
                    kind: ShotEffectKind::Compress,
                },
            ],
        );
        assert_eq!(get_abilities(&state, TARGET_ID).compressed_until, Some(Tick(60)));
        assert_eq!(get_abilities(&state, OTHER_TARGET_ID).compressed_until, Some(Tick(60)));
        assert_eq!(
            get_abilities(&state, CASTER_ID).first,
            AbilityPhase::Active { ends_at: Tick(60) },
        );
        assert_eq!(
            get_abilities(&state, CASTER_ID).shots[0],
            ShotPhase::Secreting {
                ends_at: Tick(21),
                center: WorldPoint { x: 304, y: 300 }.to_subpixel_point(),
            },
        );
    }

    #[test]
    fn press_shot_slot_freezes_from_slot_one() {
        let mut state: GameState = create_state_with_organisms(
            GameModeKind::FreeForAll,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 300 }],
        );
        get_loadout_mut(&mut state, CASTER_ID).second = SecondAbilityKind::Freeze;
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Freeze,
            WorldPoint { x: 300, y: 312 },
        );

        press(&mut state, CASTER_ID, AbilityPressSet::SECOND, None, PRESS_TICK);

        assert_eq!(get_abilities(&state, TARGET_ID).frozen_until, Some(Tick(67)));
        assert_eq!(
            get_abilities(&state, CASTER_ID).second,
            AbilityPhase::Active { ends_at: Tick(67) },
        );
    }

    #[test]
    fn press_shot_slot_restarts_the_compression_of_a_target() {
        let mut state: GameState = create_state_with_organisms(
            GameModeKind::FreeForAll,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 300 }],
        );
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        test_fixture::get_organism_mut(&mut state, TARGET_ID).abilities.compressed_until = Some(Tick(12));
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 300, y: 300 },
        );

        press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert_eq!(get_abilities(&state, TARGET_ID).compressed_until, Some(Tick(60)));
    }

    #[test]
    fn press_shot_slot_miss_keeps_the_carried_ability_ready() {
        let mut state: GameState = create_state_with_organisms(
            GameModeKind::FreeForAll,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 300 }],
        );
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 300, y: 313 },
        );

        let simulation_events: Vec<SimulationEvent> =
            press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert_eq!(simulation_events, Vec::new());
        assert_eq!(get_abilities(&state, TARGET_ID).compressed_until, None);
        assert_eq!(get_abilities(&state, CASTER_ID).first, AbilityPhase::Ready);
        assert!(matches!(
            get_abilities(&state, CASTER_ID).shots[0],
            ShotPhase::Secreting { .. },
        ));
    }

    #[test]
    fn press_shot_slot_spares_teammates_and_the_caster() {
        let mut state: GameState = create_state_with_organisms(
            GameModeKind::Skirmish,
            &[WorldPoint { x: 300, y: 294 }, WorldPoint { x: 300, y: 300 }],
        );

        for member in state.members.values_mut() {
            member.team = Some(TeamKind::Blue);
        }

        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 300, y: 297 },
        );

        let simulation_events: Vec<SimulationEvent> =
            press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert_eq!(simulation_events, Vec::new());
        assert_eq!(get_abilities(&state, CASTER_ID).compressed_until, None);
        assert_eq!(get_abilities(&state, TARGET_ID).compressed_until, None);
    }

    #[test]
    fn press_shot_slot_spares_cells_inside_the_targets_own_neutralize_field() {
        let mut state: GameState = create_state_with_organisms(
            GameModeKind::FreeForAll,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 300 }],
        );
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        let target_abilities: &mut OrganismAbilities =
            &mut test_fixture::get_organism_mut(&mut state, TARGET_ID).abilities;
        target_abilities.third = AbilityPhase::Active { ends_at: Tick(40) };
        target_abilities.third_center = Some(WorldPoint { x: 300, y: 340 });
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 300, y: 300 },
        );

        let simulation_events: Vec<SimulationEvent> =
            press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert_eq!(simulation_events, Vec::new());
    }

    #[test]
    fn press_first_does_not_reach_the_shot_while_a_compress_casters_timer_runs() {
        let mut state: GameState = create_state_with_organisms(
            GameModeKind::FreeForAll,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 300, y: 300 }],
        );
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        test_fixture::get_organism_mut(&mut state, CASTER_ID).abilities.first =
            AbilityPhase::Active { ends_at: Tick(40) };
        set_flying_shot(
            &mut state,
            CASTER_ID,
            ShotEffectKind::Compress,
            WorldPoint { x: 300, y: 300 },
        );

        press(&mut state, CASTER_ID, AbilityPressSet::FIRST, None, PRESS_TICK);

        assert!(matches!(
            get_abilities(&state, CASTER_ID).shots[0],
            ShotPhase::Flying { .. }
        ));
        assert_eq!(get_abilities(&state, TARGET_ID).compressed_until, None);
    }

    #[test]
    fn press_shot_slot_ignores_a_secreting_slot() {
        let mut state: GameState =
            create_state_with_organisms(GameModeKind::FreeForAll, &[WorldPoint { x: 100, y: 100 }]);
        get_loadout_mut(&mut state, CASTER_ID).first = FirstAbilityKind::Compress;
        let secreting_phase: ShotPhase = ShotPhase::Secreting {
            ends_at: Tick(15),
            center: SubpixelPoint { x: 0, y: 0 },
        };
        test_fixture::get_organism_mut(&mut state, CASTER_ID).abilities.shots[0] = secreting_phase;

        press(
            &mut state,
            CASTER_ID,
            AbilityPressSet::FIRST,
            Some(AimVector { x: 1, y: 1 }),
            PRESS_TICK,
        );

        assert_eq!(get_abilities(&state, CASTER_ID).shots[0], secreting_phase);
    }
```

In `shared/src/member/member.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn is_same_team_requires_one_team() {
        assert!(is_same_team(Some(TeamKind::Green), Some(TeamKind::Green)));
        assert!(!is_same_team(Some(TeamKind::Green), Some(TeamKind::Red)));
        assert!(!is_same_team(None, None));
    }
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib -- ability::activation member::member`
Expected: compilation fails; the first error is `` error[E0425]: cannot find type `WorldPoint` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/game/simulation_event.rs`, replace:

```rust
use crate::geometry::WorldPoint;
```

with:

```rust
use crate::ability::ShotEffectKind;
use crate::geometry::WorldPoint;
```

In `shared/src/game/simulation_event.rs`, replace:

```rust
        credited_to: Option<MemberId>,
    },
}
```

with:

```rust
        credited_to: Option<MemberId>,
    },
    EffectApplied {
        target: MemberId,
        caster: MemberId,
        kind: ShotEffectKind,
    },
}
```

In `shared/src/member/member.rs`, insert directly above `#[cfg(test)] mod tests {`:

```rust
/// Both on one team; members without a team are nobody's teammates.
pub fn is_same_team(first_team: Option<TeamKind>, second_team: Option<TeamKind>) -> bool {
    first_team.is_some() && first_team == second_team
}

```

In `shared/src/ability/activation.rs`, replace:

```rust
use crate::ability::ability_constants;
use crate::ability::projectile;
use crate::ability::{
    AbilityPhase, AbilityPressSet, FirstAbilityKind, Loadout, OrganismAbilities, Projectile, SecondAbilityKind,
    SporePhase,
};
use crate::game::{GameState, PlayerTickInput, SimulationEvent, Tick};
use crate::member::{Member, MemberId};
use crate::organism::Organism;

```

with:

```rust
use crate::ability;
use crate::ability::ability_constants;
use crate::ability::projectile;
use crate::ability::{
    AbilityPhase, AbilityPressSet, FirstAbilityKind, Loadout, OrganismAbilities, Projectile, SecondAbilityKind,
    ShotEffectKind, ShotPhase, SporePhase,
};
use crate::game::{GameState, PlayerTickInput, SimulationEvent, Tick};
use crate::geometry::{SubpixelPoint, WorldPoint};
use crate::member;
use crate::member::{Member, MemberId, TeamKind};
use crate::organism::Organism;

```

In `shared/src/ability/activation.rs`, replace:

```rust
fn press_first(state: &mut GameState, player_input: &PlayerTickInput, tick: Tick) -> Vec<SimulationEvent> {
    let Some((loadout, organism)) = get_loadout_and_organism(state, player_input.member_id) else {
        return Vec::new();
    };

    if !organism.abilities.first.is_ready() {
        return Vec::new();
    }

    match loadout.first {
        FirstAbilityKind::Extend => {
            organism.abilities.first = AbilityPhase::Active {
                ends_at: tick.plus(loadout.first.active_ticks()),
            };

            Vec::new()
        }
        FirstAbilityKind::Compress => Vec::new(),
    }
}
```

with:

```rust
fn press_first(state: &mut GameState, player_input: &PlayerTickInput, tick: Tick) -> Vec<SimulationEvent> {
    let Some((loadout, organism)) = get_loadout_and_organism(state, player_input.member_id) else {
        return Vec::new();
    };

    if !organism.abilities.first.is_ready() {
        return Vec::new();
    }

    match loadout.first {
        FirstAbilityKind::Extend => {
            organism.abilities.first = AbilityPhase::Active {
                ends_at: tick.plus(loadout.first.active_ticks()),
            };

            Vec::new()
        }
        FirstAbilityKind::Compress => press_shot_slot(state, player_input, ShotEffectKind::Compress, tick),
    }
}
```

In `shared/src/ability/activation.rs`, replace:

```rust
fn press_second(state: &mut GameState, player_input: &PlayerTickInput, tick: Tick) -> Vec<SimulationEvent> {
    let Some((loadout, organism)) = get_loadout_and_organism(state, player_input.member_id) else {
        return Vec::new();
    };

    if !organism.abilities.second.is_ready() {
        return Vec::new();
    }

    match loadout.second {
        SecondAbilityKind::Immortality => {
            organism.abilities.second = AbilityPhase::Active {
                ends_at: tick.plus(loadout.second.active_ticks()),
            };

            Vec::new()
        }
        SecondAbilityKind::Freeze => Vec::new(),
    }
}
```

with:

```rust
fn press_second(state: &mut GameState, player_input: &PlayerTickInput, tick: Tick) -> Vec<SimulationEvent> {
    let Some((loadout, organism)) = get_loadout_and_organism(state, player_input.member_id) else {
        return Vec::new();
    };

    if !organism.abilities.second.is_ready() {
        return Vec::new();
    }

    match loadout.second {
        SecondAbilityKind::Immortality => {
            organism.abilities.second = AbilityPhase::Active {
                ends_at: tick.plus(loadout.second.active_ticks()),
            };

            Vec::new()
        }
        SecondAbilityKind::Freeze => press_shot_slot(state, player_input, ShotEffectKind::Freeze, tick),
    }
}
```

In `shared/src/ability/activation.rs`, insert directly above `fn get_loadout_and_organism(`:

```rust
fn press_shot_slot(
    state: &mut GameState,
    player_input: &PlayerTickInput,
    effect: ShotEffectKind,
    tick: Tick,
) -> Vec<SimulationEvent> {
    let slot_index: usize = effect.slot_index();
    let Some((_, organism)) = get_loadout_and_organism(state, player_input.member_id) else {
        return Vec::new();
    };

    match organism.abilities.shots[slot_index] {
        ShotPhase::Ready => {
            let shot: Option<Projectile> = player_input.aim.and_then(|aim| projectile::launch_shot(organism, aim));

            if let Some(shot) = shot {
                organism.abilities.shots[slot_index] = ShotPhase::Flying {
                    ends_at: tick.plus(ability_constants::SHOT_FLIGHT_TICKS),
                    shot,
                };
            }

            Vec::new()
        }
        ShotPhase::Flying { shot, .. } => {
            organism.abilities.shots[slot_index] = ShotPhase::Secreting {
                ends_at: tick.plus(ability_constants::SHOT_SECRETION_TICKS),
                center: shot.position,
            };

            apply_shot_effect(state, player_input.member_id, effect, shot.position, tick)
        }
        ShotPhase::Secreting { .. } | ShotPhase::Cooling { .. } => Vec::new(),
    }
}

/// Only a hit starts the caster's carried ability timer.
fn apply_shot_effect(
    state: &mut GameState,
    caster_id: MemberId,
    effect: ShotEffectKind,
    center: SubpixelPoint,
    tick: Tick,
) -> Vec<SimulationEvent> {
    let caster_team: Option<TeamKind> = state.members.get(&caster_id).and_then(|caster| caster.team);
    let target_ids: Vec<MemberId> = state
        .members
        .values()
        .filter(|member| is_shot_target(member, caster_id, caster_team, center))
        .map(|member| member.member_id)
        .collect();
    let mut simulation_events: Vec<SimulationEvent> = Vec::new();

    for target_id in &target_ids {
        let Some((_, target_organism)) = get_loadout_and_organism(state, *target_id) else {
            continue;
        };

        let effect_until: Option<Tick> = Some(tick.plus(effect.effect_ticks()));

        match effect {
            ShotEffectKind::Compress => target_organism.abilities.compressed_until = effect_until,
            ShotEffectKind::Freeze => target_organism.abilities.frozen_until = effect_until,
        }

        simulation_events.push(SimulationEvent::EffectApplied {
            target: *target_id,
            caster: caster_id,
            kind: effect,
        });
    }

    if !target_ids.is_empty() {
        start_carried_ability_timer(state, caster_id, effect, tick);
    }

    simulation_events
}

/// Only the target's own neutralize field protects it from a shot.
fn is_shot_target(member: &Member, caster_id: MemberId, caster_team: Option<TeamKind>, center: SubpixelPoint) -> bool {
    if member.member_id == caster_id || member::is_same_team(member.team, caster_team) {
        return false;
    }

    let (Some(loadout), Some(organism)) = (member.loadout.as_ref(), member.organism.as_ref()) else {
        return false;
    };

    let neutralize_field_center: Option<WorldPoint> = organism.abilities.get_neutralize_field_center(loadout);

    organism.cells.iter().any(|lattice_coordinate| {
        let cell_center: WorldPoint = organism.cell_center(lattice_coordinate);
        let is_neutralized: bool =
            neutralize_field_center.is_some_and(|field_center| ability::is_inside_field(field_center, cell_center));

        !is_neutralized && ability::is_inside_shot_secretion(center, cell_center.to_subpixel_point())
    })
}

fn start_carried_ability_timer(state: &mut GameState, caster_id: MemberId, effect: ShotEffectKind, tick: Tick) {
    let Some((loadout, caster_organism)) = get_loadout_and_organism(state, caster_id) else {
        return;
    };

    match effect {
        ShotEffectKind::Compress => {
            caster_organism.abilities.first = AbilityPhase::Active {
                ends_at: tick.plus(loadout.first.active_ticks()),
            };
        }
        ShotEffectKind::Freeze => {
            caster_organism.abilities.second = AbilityPhase::Active {
                ends_at: tick.plus(loadout.second.active_ticks()),
            };
        }
    }
}

```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib -- ability::activation member::member`
Expected: no warnings; `test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 142 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 165 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/ability/activation.rs shared/src/game/simulation_event.rs shared/src/member/member.rs
git status --short
git commit -m "shot slots and their effects"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 13: Damage phase

**Files:**
- Create: `shared/src/ability/damage.rs`
- Modify: `shared/src/ability/mod.rs`
- Test: inline `mod tests` in `shared/src/ability/damage.rs`

Phase 9 of the tick order. For each victim (ascending id) and each acid owner (ascending id): spore secretions, then shot secretions slot 0 and slot 1, then the toxin field. Every source spares cells inside any member's neutralize field (rule 5) and the owner's teammates; spore and shot acid reach the owner's own cells (rule 1) while toxin never does. `last_hitter` is set on each actual removal only, so the last removal in iteration order wins and neutralized or teammate hits record nothing.

- [ ] **Step 1: Write the failing test**

Create `shared/src/ability/damage.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{AbilityPhase, OrganismAbilities, Projectile, ThirdAbilityKind};
    use crate::game::{GameModeKind, Tick, test_fixture};
    use crate::geometry::SubpixelVector;
    use crate::world::WorldShapeKind;

    const OWNER_ID: MemberId = MemberId(0);
    const VICTIM_ID: MemberId = MemberId(1);
    const OTHER_OWNER_ID: MemberId = MemberId(2);
    const VICTIM_POSITION: WorldPoint = WorldPoint { x: 300, y: 300 };

    fn create_state(mode: GameModeKind) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);

        for (member_id, position) in [
            (OWNER_ID, WorldPoint { x: 100, y: 100 }),
            (VICTIM_ID, VICTIM_POSITION),
            (OTHER_OWNER_ID, WorldPoint { x: 600, y: 600 }),
        ] {
            state.members.insert(
                member_id,
                test_fixture::create_participant_with_organism(member_id, position),
            );
        }

        test_fixture::get_organism_mut(&mut state, VICTIM_ID).cells.insert(LatticeCoordinate { i: 1, j: 0 });

        state
    }

    fn get_abilities_mut(state: &mut GameState, member_id: MemberId) -> &mut OrganismAbilities {
        &mut test_fixture::get_organism_mut(state, member_id).abilities
    }

    fn secrete_spore_at(state: &mut GameState, member_id: MemberId, position: WorldPoint) {
        get_abilities_mut(state, member_id).spore = SporePhase::Secreting {
            ends_at: Tick(20),
            spores: vec![Projectile {
                position: position.to_subpixel_point(),
                velocity: SubpixelVector { x: 0, y: 0 },
            }],
        };
    }

    fn secrete_shot_at(state: &mut GameState, member_id: MemberId, slot_index: usize, position: WorldPoint) {
        get_abilities_mut(state, member_id).shots[slot_index] = ShotPhase::Secreting {
            ends_at: Tick(20),
            center: position.to_subpixel_point(),
        };
    }

    fn activate_field(state: &mut GameState, member_id: MemberId, third: ThirdAbilityKind, center: WorldPoint) {
        let loadout: &mut Loadout = state.members.get_mut(&member_id).unwrap().loadout.as_mut().unwrap();
        loadout.third = third;

        let abilities: &mut OrganismAbilities = get_abilities_mut(state, member_id);
        abilities.third = AbilityPhase::Active { ends_at: Tick(20) };
        abilities.third_center = Some(center);
    }

    fn get_cells(state: &GameState, member_id: MemberId) -> Vec<LatticeCoordinate> {
        test_fixture::get_organism(state, member_id).cells.iter().collect()
    }

    #[test]
    fn run_damage_phase_removes_cells_inside_a_spore_secretion() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, OWNER_ID, WorldPoint { x: 280, y: 300 });

        run_damage_phase(&mut state);

        assert_eq!(get_cells(&state, VICTIM_ID), vec![LatticeCoordinate { i: 1, j: 0 }]);
        assert_eq!(
            test_fixture::get_organism(&state, VICTIM_ID).last_hitter,
            Some(OWNER_ID)
        );
    }

    #[test]
    fn run_damage_phase_removes_cells_inside_a_shot_secretion() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_shot_at(&mut state, OWNER_ID, 1, WorldPoint { x: 318, y: 300 });

        run_damage_phase(&mut state);

        assert_eq!(get_cells(&state, VICTIM_ID), vec![LatticeCoordinate { i: 0, j: 0 }]);
    }

    #[test]
    fn run_damage_phase_lets_acid_reach_its_owner() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, VICTIM_ID, VICTIM_POSITION);

        run_damage_phase(&mut state);

        let victim_organism: &Organism = test_fixture::get_organism(&state, VICTIM_ID);
        assert!(victim_organism.cells.is_empty());
        assert_eq!(victim_organism.last_hitter, Some(VICTIM_ID));
    }

    #[test]
    fn run_damage_phase_keeps_toxin_off_its_owner() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        activate_field(&mut state, VICTIM_ID, ThirdAbilityKind::Toxin, VICTIM_POSITION);
        activate_field(
            &mut state,
            OWNER_ID,
            ThirdAbilityKind::Toxin,
            WorldPoint { x: 245, y: 300 },
        );

        run_damage_phase(&mut state);

        assert_eq!(get_cells(&state, VICTIM_ID), vec![LatticeCoordinate { i: 1, j: 0 }]);
        assert_eq!(get_cells(&state, OWNER_ID), vec![LatticeCoordinate { i: 0, j: 0 }]);
    }

    #[test]
    fn run_damage_phase_spares_cells_inside_any_neutralize_field() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, OWNER_ID, VICTIM_POSITION);
        activate_field(
            &mut state,
            OTHER_OWNER_ID,
            ThirdAbilityKind::Neutralize,
            WorldPoint { x: 245, y: 300 },
        );

        run_damage_phase(&mut state);

        assert_eq!(get_cells(&state, VICTIM_ID), vec![LatticeCoordinate { i: 0, j: 0 }]);
    }

    #[test]
    fn run_damage_phase_spares_teammates_of_the_owner() {
        let mut state: GameState = create_state(GameModeKind::Skirmish);

        for member in state.members.values_mut() {
            member.team = Some(TeamKind::Red);
        }

        secrete_spore_at(&mut state, OWNER_ID, VICTIM_POSITION);
        activate_field(&mut state, OWNER_ID, ThirdAbilityKind::Toxin, VICTIM_POSITION);

        run_damage_phase(&mut state);

        let victim_organism: &Organism = test_fixture::get_organism(&state, VICTIM_ID);
        assert_eq!(victim_organism.cells.count(), 2);
        assert_eq!(victim_organism.last_hitter, None);
    }

    #[test]
    fn run_damage_phase_credits_the_last_owner_in_id_order() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, OTHER_OWNER_ID, WorldPoint { x: 330, y: 300 });
        activate_field(
            &mut state,
            OWNER_ID,
            ThirdAbilityKind::Toxin,
            WorldPoint { x: 245, y: 300 },
        );

        run_damage_phase(&mut state);

        let victim_organism: &Organism = test_fixture::get_organism(&state, VICTIM_ID);
        assert!(victim_organism.cells.is_empty());
        assert_eq!(victim_organism.last_hitter, Some(OTHER_OWNER_ID));
    }

    #[test]
    fn run_damage_phase_records_no_hitter_for_neutralized_acid() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, OWNER_ID, VICTIM_POSITION);
        activate_field(&mut state, VICTIM_ID, ThirdAbilityKind::Neutralize, VICTIM_POSITION);

        run_damage_phase(&mut state);

        let victim_organism: &Organism = test_fixture::get_organism(&state, VICTIM_ID);
        assert_eq!(victim_organism.cells.count(), 2);
        assert_eq!(victim_organism.last_hitter, None);
    }
}
```

Replace the whole of `shared/src/ability/mod.rs` with:

```rust
pub mod ability_constants;
pub mod ability_model;
pub mod activation;
pub mod damage;
pub mod projectile;

pub use ability_constants::*;
pub use ability_model::*;
pub use activation::*;
pub use damage::*;
pub use projectile::*;
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib ability::damage`
Expected: compilation fails; the first error is `` error[E0425]: cannot find type `MemberId` in this scope ``.

- [ ] **Step 3: Implement**

Replace the whole of `shared/src/ability/damage.rs` with:

```rust
use crate::ability;
use crate::ability::{Loadout, ShotPhase, SporePhase};
use crate::game::GameState;
use crate::geometry::{LatticeCoordinate, SubpixelPoint, WorldPoint};
use crate::member;
use crate::member::{Member, MemberId, TeamKind};
use crate::organism::Organism;

struct AcidSource {
    owner_id: MemberId,
    owner_team: Option<TeamKind>,
    spore_secretion_positions: Vec<SubpixelPoint>,
    /// Slot 0, then slot 1.
    shot_secretion_centers: Vec<SubpixelPoint>,
    toxin_field_center: Option<WorldPoint>,
}

impl AcidSource {
    fn from_member(member: &Member) -> Option<AcidSource> {
        let loadout: &Loadout = member.loadout.as_ref()?;
        let organism: &Organism = member.organism.as_ref()?;
        let spore_secretion_positions: Vec<SubpixelPoint> = match &organism.abilities.spore {
            SporePhase::Secreting { spores, .. } => spores.iter().map(|spore| spore.position).collect(),
            SporePhase::Ready | SporePhase::Flying { .. } | SporePhase::Cooling { .. } => Vec::new(),
        };
        let shot_secretion_centers: Vec<SubpixelPoint> = organism
            .abilities
            .shots
            .iter()
            .filter_map(|shot_phase| match shot_phase {
                ShotPhase::Secreting { center, .. } => Some(*center),
                ShotPhase::Ready | ShotPhase::Flying { .. } | ShotPhase::Cooling { .. } => None,
            })
            .collect();

        Some(AcidSource {
            owner_id: member.member_id,
            owner_team: member.team,
            spore_secretion_positions,
            shot_secretion_centers,
            toxin_field_center: organism.abilities.get_toxin_field_center(loadout),
        })
    }

    /// Spore and shot acid reach the owner's own cells; toxin never does.
    fn removes_cell(&self, victim_id: MemberId, cell_center: WorldPoint) -> bool {
        let cell_center_subpixels: SubpixelPoint = cell_center.to_subpixel_point();
        let is_in_spore_secretion: bool = self
            .spore_secretion_positions
            .iter()
            .any(|spore_position| ability::is_inside_spore_secretion(*spore_position, cell_center_subpixels));
        let is_in_shot_secretion: bool = self
            .shot_secretion_centers
            .iter()
            .any(|shot_center| ability::is_inside_shot_secretion(*shot_center, cell_center_subpixels));
        let is_in_toxin_field: bool = victim_id != self.owner_id
            && self
                .toxin_field_center
                .is_some_and(|field_center| ability::is_inside_field(field_center, cell_center));

        is_in_spore_secretion || is_in_shot_secretion || is_in_toxin_field
    }
}

/// Removes every cell inside an acid source of a non-teammate and outside every neutralize field.
pub fn run_damage_phase(state: &mut GameState) {
    let acid_sources: Vec<AcidSource> = state.members.values().filter_map(AcidSource::from_member).collect();
    let neutralize_field_centers: Vec<WorldPoint> = get_neutralize_field_centers(state);

    for member in state.members.values_mut() {
        damage_member(member, &acid_sources, &neutralize_field_centers);
    }
}

/// The owner of the last removal, in owner order, is the last hitter.
fn damage_member(member: &mut Member, acid_sources: &[AcidSource], neutralize_field_centers: &[WorldPoint]) {
    let victim_id: MemberId = member.member_id;
    let victim_team: Option<TeamKind> = member.team;
    let Some(organism) = member.organism.as_mut() else {
        return;
    };

    for acid_source in acid_sources {
        let is_teammate: bool =
            victim_id != acid_source.owner_id && member::is_same_team(victim_team, acid_source.owner_team);

        if is_teammate {
            continue;
        }

        let lattice_coordinates: Vec<LatticeCoordinate> = organism.cells.iter().collect();

        for lattice_coordinate in lattice_coordinates {
            let cell_center: WorldPoint = organism.cell_center(lattice_coordinate);
            let is_neutralized: bool = neutralize_field_centers
                .iter()
                .any(|field_center| ability::is_inside_field(*field_center, cell_center));

            if is_neutralized || !acid_source.removes_cell(victim_id, cell_center) {
                continue;
            }

            organism.cells.remove(lattice_coordinate);
            organism.last_hitter = Some(acid_source.owner_id);
        }
    }
}

fn get_neutralize_field_centers(state: &GameState) -> Vec<WorldPoint> {
    state
        .members
        .values()
        .filter_map(|member| {
            let loadout: &Loadout = member.loadout.as_ref()?;
            let organism: &Organism = member.organism.as_ref()?;

            organism.abilities.get_neutralize_field_center(loadout)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{AbilityPhase, OrganismAbilities, Projectile, ThirdAbilityKind};
    use crate::game::{GameModeKind, Tick, test_fixture};
    use crate::geometry::SubpixelVector;
    use crate::world::WorldShapeKind;

    const OWNER_ID: MemberId = MemberId(0);
    const VICTIM_ID: MemberId = MemberId(1);
    const OTHER_OWNER_ID: MemberId = MemberId(2);
    const VICTIM_POSITION: WorldPoint = WorldPoint { x: 300, y: 300 };

    fn create_state(mode: GameModeKind) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);

        for (member_id, position) in [
            (OWNER_ID, WorldPoint { x: 100, y: 100 }),
            (VICTIM_ID, VICTIM_POSITION),
            (OTHER_OWNER_ID, WorldPoint { x: 600, y: 600 }),
        ] {
            state.members.insert(
                member_id,
                test_fixture::create_participant_with_organism(member_id, position),
            );
        }

        test_fixture::get_organism_mut(&mut state, VICTIM_ID).cells.insert(LatticeCoordinate { i: 1, j: 0 });

        state
    }

    fn get_abilities_mut(state: &mut GameState, member_id: MemberId) -> &mut OrganismAbilities {
        &mut test_fixture::get_organism_mut(state, member_id).abilities
    }

    fn secrete_spore_at(state: &mut GameState, member_id: MemberId, position: WorldPoint) {
        get_abilities_mut(state, member_id).spore = SporePhase::Secreting {
            ends_at: Tick(20),
            spores: vec![Projectile {
                position: position.to_subpixel_point(),
                velocity: SubpixelVector { x: 0, y: 0 },
            }],
        };
    }

    fn secrete_shot_at(state: &mut GameState, member_id: MemberId, slot_index: usize, position: WorldPoint) {
        get_abilities_mut(state, member_id).shots[slot_index] = ShotPhase::Secreting {
            ends_at: Tick(20),
            center: position.to_subpixel_point(),
        };
    }

    fn activate_field(state: &mut GameState, member_id: MemberId, third: ThirdAbilityKind, center: WorldPoint) {
        let loadout: &mut Loadout = state.members.get_mut(&member_id).unwrap().loadout.as_mut().unwrap();
        loadout.third = third;

        let abilities: &mut OrganismAbilities = get_abilities_mut(state, member_id);
        abilities.third = AbilityPhase::Active { ends_at: Tick(20) };
        abilities.third_center = Some(center);
    }

    fn get_cells(state: &GameState, member_id: MemberId) -> Vec<LatticeCoordinate> {
        test_fixture::get_organism(state, member_id).cells.iter().collect()
    }

    #[test]
    fn run_damage_phase_removes_cells_inside_a_spore_secretion() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, OWNER_ID, WorldPoint { x: 280, y: 300 });

        run_damage_phase(&mut state);

        assert_eq!(get_cells(&state, VICTIM_ID), vec![LatticeCoordinate { i: 1, j: 0 }]);
        assert_eq!(
            test_fixture::get_organism(&state, VICTIM_ID).last_hitter,
            Some(OWNER_ID)
        );
    }

    #[test]
    fn run_damage_phase_removes_cells_inside_a_shot_secretion() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_shot_at(&mut state, OWNER_ID, 1, WorldPoint { x: 318, y: 300 });

        run_damage_phase(&mut state);

        assert_eq!(get_cells(&state, VICTIM_ID), vec![LatticeCoordinate { i: 0, j: 0 }]);
    }

    #[test]
    fn run_damage_phase_lets_acid_reach_its_owner() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, VICTIM_ID, VICTIM_POSITION);

        run_damage_phase(&mut state);

        let victim_organism: &Organism = test_fixture::get_organism(&state, VICTIM_ID);
        assert!(victim_organism.cells.is_empty());
        assert_eq!(victim_organism.last_hitter, Some(VICTIM_ID));
    }

    #[test]
    fn run_damage_phase_keeps_toxin_off_its_owner() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        activate_field(&mut state, VICTIM_ID, ThirdAbilityKind::Toxin, VICTIM_POSITION);
        activate_field(
            &mut state,
            OWNER_ID,
            ThirdAbilityKind::Toxin,
            WorldPoint { x: 245, y: 300 },
        );

        run_damage_phase(&mut state);

        assert_eq!(get_cells(&state, VICTIM_ID), vec![LatticeCoordinate { i: 1, j: 0 }]);
        assert_eq!(get_cells(&state, OWNER_ID), vec![LatticeCoordinate { i: 0, j: 0 }]);
    }

    #[test]
    fn run_damage_phase_spares_cells_inside_any_neutralize_field() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, OWNER_ID, VICTIM_POSITION);
        activate_field(
            &mut state,
            OTHER_OWNER_ID,
            ThirdAbilityKind::Neutralize,
            WorldPoint { x: 245, y: 300 },
        );

        run_damage_phase(&mut state);

        assert_eq!(get_cells(&state, VICTIM_ID), vec![LatticeCoordinate { i: 0, j: 0 }]);
    }

    #[test]
    fn run_damage_phase_spares_teammates_of_the_owner() {
        let mut state: GameState = create_state(GameModeKind::Skirmish);

        for member in state.members.values_mut() {
            member.team = Some(TeamKind::Red);
        }

        secrete_spore_at(&mut state, OWNER_ID, VICTIM_POSITION);
        activate_field(&mut state, OWNER_ID, ThirdAbilityKind::Toxin, VICTIM_POSITION);

        run_damage_phase(&mut state);

        let victim_organism: &Organism = test_fixture::get_organism(&state, VICTIM_ID);
        assert_eq!(victim_organism.cells.count(), 2);
        assert_eq!(victim_organism.last_hitter, None);
    }

    #[test]
    fn run_damage_phase_credits_the_last_owner_in_id_order() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, OTHER_OWNER_ID, WorldPoint { x: 330, y: 300 });
        activate_field(
            &mut state,
            OWNER_ID,
            ThirdAbilityKind::Toxin,
            WorldPoint { x: 245, y: 300 },
        );

        run_damage_phase(&mut state);

        let victim_organism: &Organism = test_fixture::get_organism(&state, VICTIM_ID);
        assert!(victim_organism.cells.is_empty());
        assert_eq!(victim_organism.last_hitter, Some(OTHER_OWNER_ID));
    }

    #[test]
    fn run_damage_phase_records_no_hitter_for_neutralized_acid() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_spore_at(&mut state, OWNER_ID, VICTIM_POSITION);
        activate_field(&mut state, VICTIM_ID, ThirdAbilityKind::Neutralize, VICTIM_POSITION);

        run_damage_phase(&mut state);

        let victim_organism: &Organism = test_fixture::get_organism(&state, VICTIM_ID);
        assert_eq!(victim_organism.cells.count(), 2);
        assert_eq!(victim_organism.last_hitter, None);
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib ability::damage`
Expected: no warnings; `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 165 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 173 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/ability/damage.rs shared/src/ability/mod.rs
git status --short
git commit -m "damage phase"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 14: Step runs the ability phases

**Files:**
- Modify: `shared/src/game/step.rs`
- Test: inline `mod tests` in `shared/src/game/step.rs`

`step` gains timer expiry (phase 3), ability presses (phase 4, whose `EffectApplied` events follow the membership events), projectile flight (phase 5) and damage (phase 9). Expiry precedes presses, so a cooldown ending this tick can be pressed this tick; flight follows presses, so a projectile moves in its launch tick; damage precedes death bookkeeping, so an acid kill is credited in the same tick.

- [ ] **Step 1: Write the failing test**

In `shared/src/game/step.rs`, replace:

```rust
    use super::*;
    use crate::ability::AbilityPressSet;
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, WorldPoint};
    use crate::member::{MemberRoleKind, OrganismColorKind, SkinKind, TeamKind};
    use crate::organism::CellOccupancy;
    use crate::world::WorldShapeKind;
```

with:

```rust
    use super::*;
    use crate::ability::{AbilityPhase, AbilityPressSet, Projectile, SporePhase};
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, WorldPoint};
    use crate::member::{MemberRoleKind, OrganismColorKind, SkinKind, TeamKind};
    use crate::organism::CellOccupancy;
    use crate::world::WorldShapeKind;
```

In `shared/src/game/step.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    fn create_press_input(member_id: MemberId, ability_presses: AbilityPressSet) -> PlayerTickInput {
        PlayerTickInput {
            member_id,
            cursor: WorldPoint { x: 100, y: 100 },
            ability_presses,
            aim: None,
        }
    }

    #[test]
    fn step_expires_a_cooldown_before_the_press_of_the_same_tick() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0)]);
        freeze(&mut state, MemberId(0));
        test_fixture::get_organism_mut(&mut state, MemberId(0)).abilities.first =
            AbilityPhase::Cooling { ready_at: Tick(1) };
        let mut bundle: InputBundle = create_bundle(1, Vec::new());
        bundle.player_inputs = vec![create_press_input(MemberId(0), AbilityPressSet::FIRST)];

        step(&mut state, &bundle).unwrap();

        assert_eq!(
            test_fixture::get_organism(&state, MemberId(0)).abilities.first,
            AbilityPhase::Active { ends_at: Tick(65) },
        );
    }

    #[test]
    fn step_moves_spores_in_the_tick_they_launch() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0)]);
        freeze(&mut state, MemberId(0));
        let organism: &mut Organism = test_fixture::get_organism_mut(&mut state, MemberId(0));

        for j in -1..=1 {
            for i in -1..=1 {
                organism.cells.insert(LatticeCoordinate { i, j });
            }
        }

        let mut bundle: InputBundle = create_bundle(1, Vec::new());
        bundle.player_inputs = vec![create_press_input(MemberId(0), AbilityPressSet::FOURTH)];

        step(&mut state, &bundle).unwrap();

        let expected_first_spore: Projectile = Projectile {
            position: SubpixelPoint {
                x: 96_256 - 7603,
                y: 96_256 - 7603,
            },
            velocity: SubpixelVector { x: -7603, y: -7603 },
        };

        assert!(matches!(
            &test_fixture::get_organism(&state, MemberId(0)).abilities.spore,
            SporePhase::Flying { ends_at: Tick(25), spores } if spores.len() == 8 && spores[0] == expected_first_spore,
        ));
    }

    #[test]
    fn step_damages_before_recording_deaths() {
        let mut state: GameState = create_state_with_organisms(&[MemberId(0), MemberId(1)]);
        freeze(&mut state, MemberId(0));
        freeze(&mut state, MemberId(1));
        test_fixture::get_organism_mut(&mut state, MemberId(0)).abilities.spore = SporePhase::Secreting {
            ends_at: Tick(5),
            spores: vec![Projectile {
                position: WorldPoint { x: 300, y: 100 }.to_subpixel_point(),
                velocity: SubpixelVector { x: 0, y: 0 },
            }],
        };

        let simulation_events: Vec<SimulationEvent> = step(&mut state, &create_bundle(1, Vec::new())).unwrap();

        assert_eq!(
            simulation_events,
            vec![SimulationEvent::OrganismDied {
                member_id: MemberId(1),
                credited_to: Some(MemberId(0)),
            }],
        );
        assert_eq!(state.members[&MemberId(0)].score.kills, 1);
    }
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib game::step`
Expected: `test result: FAILED. 15 passed; 3 failed`; the failing tests are `game::step::tests::step_damages_before_recording_deaths`, `game::step::tests::step_expires_a_cooldown_before_the_press_of_the_same_tick`, `game::step::tests::step_moves_spores_in_the_tick_they_launch`.

- [ ] **Step 3: Implement**

In `shared/src/game/step.rs`, replace:

```rust
use crate::game::{GameState, InputBundle, MemberEvent, PlayerTickInput, SimulationEvent, Tick};
use crate::member::{Appearance, Member, MemberId, Score};
use crate::organism::Organism;
use crate::organism::{growth, spawn};
```

with:

```rust
use crate::ability::{activation, damage, projectile};
use crate::game::{GameState, InputBundle, MemberEvent, PlayerTickInput, SimulationEvent, Tick};
use crate::member::{Appearance, Member, MemberId, Score};
use crate::organism::Organism;
use crate::organism::{growth, spawn};
```

In `shared/src/game/step.rs`, replace:

```rust
    apply_cursor_updates(state, &bundle.player_inputs);
    growth::run_birth_phase(state, bundle.tick);
    growth::run_natural_death_phase(state);

    let death_events: Vec<SimulationEvent> = record_deaths(state);
    simulation_events.extend(death_events);

    retighten_cell_occupancies(state);
```

with:

```rust
    apply_cursor_updates(state, &bundle.player_inputs);
    activation::run_timer_expiry_phase(state, bundle.tick);

    let effect_events: Vec<SimulationEvent> =
        activation::run_ability_press_phase(state, &bundle.player_inputs, bundle.tick);
    simulation_events.extend(effect_events);

    projectile::run_flight_phase(state);
    growth::run_birth_phase(state, bundle.tick);
    growth::run_natural_death_phase(state);
    damage::run_damage_phase(state);

    let death_events: Vec<SimulationEvent> = record_deaths(state);
    simulation_events.extend(death_events);

    retighten_cell_occupancies(state);
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib game::step`
Expected: no warnings; `test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 158 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 176 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/game/step.rs
git status --short
git commit -m "step runs the ability phases"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 15: Force spawn

**Files:**
- Modify: `shared/src/organism/spawn.rs`
- Test: inline `mod tests` in `shared/src/organism/spawn.rs`

Rounds force spawn every Participant: all organisms are discarded without a death, then Participants with a loadout spawn in ascending id while the alive count is below the player cap; pure Spectators never spawn. The search-and-place half of `spawn_member` becomes `spawn_at_found_position`, shared by both; `spawn_member` now converts the member before the search, which draws the same numbers as before.

- [ ] **Step 1: Write the failing test**

In `shared/src/organism/spawn.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    fn create_survival_state_with_participants(participant_count: u32) -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Rectangle, 800);

        for index in 0..participant_count {
            let member_id: MemberId = MemberId(index);
            state.members.insert(member_id, test_fixture::create_participant(member_id));
        }

        state
    }

    #[test]
    fn force_spawn_participants_replaces_organisms_without_deaths() {
        let mut state: GameState = create_survival_state_with_participants(2);
        place_organism(&mut state, MemberId(0), WorldPoint { x: 9, y: 9 }).unwrap();
        test_fixture::get_organism_mut(&mut state, MemberId(0))
            .cells
            .insert(LatticeCoordinate { i: 1, j: 0 });

        let simulation_events: Vec<SimulationEvent> = force_spawn_participants(&mut state);

        let cursors: Vec<WorldPoint> = [MemberId(0), MemberId(1)]
            .iter()
            .map(|member_id| test_fixture::get_organism(&state, *member_id).cursor)
            .collect();
        assert_eq!(
            simulation_events,
            vec![
                SimulationEvent::OrganismSpawned {
                    member_id: MemberId(0),
                    cursor: cursors[0],
                },
                SimulationEvent::OrganismSpawned {
                    member_id: MemberId(1),
                    cursor: cursors[1],
                },
            ],
        );
        assert_ne!(cursors[0], WorldPoint { x: 9, y: 9 });
        assert_eq!(test_fixture::get_organism(&state, MemberId(0)).cells.count(), 1);
        assert_eq!(state.members[&MemberId(0)].score.deaths, 0);
    }

    #[test]
    fn force_spawn_participants_stops_at_the_player_cap() {
        let mut state: GameState = create_survival_state_with_participants(3);
        state.settings.player_cap = 2;

        let simulation_events: Vec<SimulationEvent> = force_spawn_participants(&mut state);

        assert_eq!(simulation_events.len(), 2);
        assert_eq!(state.alive_organism_count(), 2);
        assert_eq!(state.members[&MemberId(2)].organism, None);
    }

    #[test]
    fn force_spawn_participants_never_spawns_a_spectator() {
        let mut state: GameState = create_survival_state_with_participants(1);
        let mut spectator: Member = test_fixture::create_participant(MemberId(1));
        spectator.role = MemberRoleKind::Spectator;
        spectator.loadout = None;
        state.members.insert(MemberId(1), spectator);

        force_spawn_participants(&mut state);

        assert!(state.members[&MemberId(0)].organism.is_some());
        assert_eq!(state.members[&MemberId(1)].organism, None);
    }
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib organism::spawn`
Expected: compilation fails; the first error is `` error[E0425]: cannot find function `force_spawn_participants` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/organism/spawn.rs`, replace:

```rust
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
```

with:

```rust
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

    let member: &mut Member = state.members.get_mut(&member_id)?;
    member.role = MemberRoleKind::Participant;
    member.loadout = Some(loadout.with_team_color(team));
    member.team = team;

    Some(spawn_at_found_position(state, member_id))
}

/// Every organism is discarded without a death; then each Participant with a loadout, in ascending id, spawns while
/// the alive organism count is below the player cap.
pub fn force_spawn_participants(state: &mut GameState) -> Vec<SimulationEvent> {
    for member in state.members.values_mut() {
        member.organism = None;
    }

    let participant_ids: Vec<MemberId> = state
        .members
        .values()
        .filter(|member| member.role == MemberRoleKind::Participant && member.loadout.is_some())
        .map(|member| member.member_id)
        .collect();
    let mut simulation_events: Vec<SimulationEvent> = Vec::new();

    for member_id in participant_ids {
        let is_at_player_cap: bool = state.alive_organism_count() >= u32::from(state.settings.player_cap);

        if is_at_player_cap {
            break;
        }

        simulation_events.push(spawn_at_found_position(state, member_id));
    }

    simulation_events
}
```

In `shared/src/organism/spawn.rs`, insert directly above `fn get_spawn_rejection(`:

```rust
fn spawn_at_found_position(state: &mut GameState, member_id: MemberId) -> SimulationEvent {
    let spawn_position: Option<WorldPoint> = find_spawn_position(&mut state.rng, &state.world, &state.members);
    let member: Option<&mut Member> = state.members.get_mut(&member_id);

    let (Some(position), Some(member)) = (spawn_position, member) else {
        return SimulationEvent::SpawnRejected {
            member_id,
            reason: SpawnRejectionKind::PositionNotFound,
        };
    };

    member.organism = Some(Organism::new(position));

    SimulationEvent::OrganismSpawned {
        member_id,
        cursor: position,
    }
}

```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib organism::spawn`
Expected: no warnings; `test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 158 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 179 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/organism/spawn.rs
git status --short
git commit -m "force spawn"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 16: Round transitions and countdown

**Files:**
- Modify: `shared/src/round/round.rs`
- Modify: `shared/src/game/simulation_event.rs`
- Test: inline `mod tests` in `shared/src/round/round.rs`

The survival round machine of the design's table, at most one transition per tick, with one participant predicate (members with the Participant role) for starting and cancelling. Force spawns happen at 100 ticks into PreRound and on leaving PostRound, which also restores the whole initial world. The countdown is computed from ticks.

- [ ] **Step 1: Write the failing test**

Append to the end of `shared/src/round/round.rs` (after a blank line):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::world::{WorldBounds, WorldShapeKind};

    const ROUND_START_TICK: Tick = Tick(1000);

    fn create_survival_state(participant_count: u32, phase: RoundPhase) -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Rectangle, 800);
        state.round = Some(RoundState {
            phase,
            phase_started_at: ROUND_START_TICK,
        });

        for index in 0..participant_count {
            let member_id: MemberId = MemberId(index);
            state.members.insert(member_id, test_fixture::create_participant(member_id));
        }

        state
    }

    fn give_organisms(state: &mut GameState, member_ids: &[MemberId]) {
        for (index, member_id) in member_ids.iter().enumerate() {
            let x: i32 = 100 + 100 * i32::try_from(index).unwrap();
            spawn::place_organism(state, *member_id, WorldPoint { x, y: 100 }).unwrap();
        }
    }

    fn get_phase(state: &GameState) -> RoundPhase {
        state.round.unwrap().phase
    }

    fn get_tick_after(elapsed_ticks: u32) -> Tick {
        ROUND_START_TICK.plus(elapsed_ticks)
    }

    #[test]
    fn delays_match_the_designed_tick_counts() {
        assert_eq!(FORCE_SPAWN_ELAPSED_TICKS, 100);
        assert_eq!(ROUND_DELAY_TICKS, 114);
    }

    #[test]
    fn get_countdown_seconds_counts_down_the_delay() {
        let round: RoundState = RoundState {
            phase: RoundPhase::PreRound,
            phase_started_at: ROUND_START_TICK,
        };
        let playing_round: RoundState = RoundState {
            phase: RoundPhase::Playing,
            phase_started_at: ROUND_START_TICK,
        };

        assert_eq!(round.get_countdown_seconds(ROUND_START_TICK), Some(8));
        assert_eq!(round.get_countdown_seconds(get_tick_after(14)), Some(7));
        assert_eq!(round.get_countdown_seconds(get_tick_after(15)), Some(7));
        assert_eq!(round.get_countdown_seconds(get_tick_after(114)), Some(0));
        assert_eq!(playing_round.get_countdown_seconds(ROUND_START_TICK), None);
    }

    #[test]
    fn run_round_transition_phase_starts_the_pre_round_at_the_player_minimum() {
        let mut state: GameState = create_survival_state(1, RoundPhase::Waiting);

        assert_eq!(run_round_transition_phase(&mut state, Tick(1001)), Vec::new());

        state.members.insert(MemberId(1), test_fixture::create_participant(MemberId(1)));

        assert_eq!(
            run_round_transition_phase(&mut state, Tick(1002)),
            vec![SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::PreRound,
            }],
        );
        assert_eq!(
            state.round,
            Some(RoundState {
                phase: RoundPhase::PreRound,
                phase_started_at: Tick(1002),
            }),
        );
    }

    #[test]
    fn run_round_transition_phase_counts_spectators_out() {
        let mut state: GameState = create_survival_state(1, RoundPhase::Waiting);
        let mut spectator: Member = test_fixture::create_participant(MemberId(1));
        spectator.role = MemberRoleKind::Spectator;
        state.members.insert(MemberId(1), spectator);

        assert_eq!(run_round_transition_phase(&mut state, Tick(1001)), Vec::new());
        assert_eq!(get_phase(&state), RoundPhase::Waiting);
    }

    #[test]
    fn run_round_transition_phase_cancels_the_pre_round_below_the_minimum() {
        let mut state: GameState = create_survival_state(1, RoundPhase::PreRound);

        assert_eq!(
            run_round_transition_phase(&mut state, get_tick_after(50)),
            vec![SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::Waiting,
            }],
        );
    }

    #[test]
    fn run_round_transition_phase_force_spawns_during_the_pre_round() {
        let mut state: GameState = create_survival_state(2, RoundPhase::PreRound);

        assert_eq!(run_round_transition_phase(&mut state, get_tick_after(99)), Vec::new());

        let simulation_events: Vec<SimulationEvent> = run_round_transition_phase(&mut state, get_tick_after(100));

        assert_eq!(simulation_events.len(), 2);
        assert_eq!(state.alive_organism_count(), 2);
        assert_eq!(get_phase(&state), RoundPhase::PreRound);
    }

    #[test]
    fn run_round_transition_phase_starts_playing_after_the_delay() {
        let mut state: GameState = create_survival_state(2, RoundPhase::PreRound);

        assert_eq!(run_round_transition_phase(&mut state, get_tick_after(113)), Vec::new());
        assert_eq!(
            run_round_transition_phase(&mut state, get_tick_after(114)),
            vec![SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::Playing,
            }],
        );
    }

    #[test]
    fn run_round_transition_phase_gives_the_sole_survivor_a_win() {
        let mut state: GameState = create_survival_state(3, RoundPhase::Playing);
        give_organisms(&mut state, &[MemberId(0), MemberId(2)]);

        assert_eq!(run_round_transition_phase(&mut state, Tick(1001)), Vec::new());

        state.members.get_mut(&MemberId(0)).unwrap().organism = None;

        assert_eq!(
            run_round_transition_phase(&mut state, Tick(1002)),
            vec![
                SimulationEvent::RoundPhaseChanged {
                    phase: RoundPhase::PostRound,
                },
                SimulationEvent::RoundWon { member_id: MemberId(2) },
            ],
        );
        assert_eq!(state.members[&MemberId(2)].score.wins, 1);
    }

    #[test]
    fn run_round_transition_phase_ends_without_a_winner_when_nobody_survives() {
        let mut state: GameState = create_survival_state(2, RoundPhase::Playing);

        assert_eq!(
            run_round_transition_phase(&mut state, Tick(1001)),
            vec![SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::PostRound,
            }],
        );
        assert!(state.members.values().all(|member| member.score.wins == 0));
    }

    #[test]
    fn run_round_transition_phase_restores_the_world_and_force_spawns_after_the_post_round() {
        let mut state: GameState = create_survival_state(2, RoundPhase::PostRound);
        state.world.shrink();

        assert_eq!(run_round_transition_phase(&mut state, get_tick_after(113)), Vec::new());

        let simulation_events: Vec<SimulationEvent> = run_round_transition_phase(&mut state, get_tick_after(114));

        assert_eq!(
            simulation_events[0],
            SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::Waiting,
            },
        );
        assert_eq!(simulation_events.len(), 3);
        assert_eq!(state.world.bounds, WorldBounds::from_pixel_size(800, 800));
        assert_eq!(state.alive_organism_count(), 2);
        assert_eq!(get_phase(&state), RoundPhase::Waiting);
    }

    #[test]
    fn run_round_transition_phase_does_nothing_without_rounds() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);

        assert_eq!(run_round_transition_phase(&mut state, Tick(1)), Vec::new());
    }

    #[test]
    fn run_survival_shrink_phase_shrinks_only_while_playing() {
        let mut waiting_state: GameState = create_survival_state(0, RoundPhase::Waiting);
        let mut playing_state: GameState = create_survival_state(0, RoundPhase::Playing);

        run_survival_shrink_phase(&mut waiting_state);
        run_survival_shrink_phase(&mut playing_state);

        assert_eq!(waiting_state.world.bounds, waiting_state.world.initial_bounds);
        assert_ne!(playing_state.world.bounds, playing_state.world.initial_bounds);
    }
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib round::round`
Expected: compilation fails; the first error is `` error[E0425]: cannot find type `GameState` in this scope ``.

- [ ] **Step 3: Implement**

Replace the whole of `shared/src/round/round.rs` with:

```rust
use crate::game::tick;
use crate::game::{GameState, SimulationEvent, Tick};
use crate::member::{Member, MemberId, MemberRoleKind};
use crate::organism::spawn;

pub const FORCE_SPAWN_ELAPSED_TICKS: u32 = tick::get_ticks_from_milliseconds(7000);
/// Length of the PreRound and PostRound phases.
pub const ROUND_DELAY_TICKS: u32 = tick::get_ticks_from_milliseconds(8000);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoundState {
    pub phase: RoundPhase,
    pub phase_started_at: Tick,
}

impl RoundState {
    /// Seconds left of the PreRound or PostRound delay; `None` in the other phases.
    pub fn get_countdown_seconds(&self, tick: Tick) -> Option<u32> {
        match self.phase {
            RoundPhase::PreRound | RoundPhase::PostRound => {
                let elapsed_ticks: u32 = tick.ticks_since(self.phase_started_at);
                let remaining_ticks: u32 = ROUND_DELAY_TICKS.saturating_sub(elapsed_ticks);

                Some(tick::get_seconds_rounded_up(remaining_ticks))
            }
            RoundPhase::Waiting | RoundPhase::Playing => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundPhase {
    Waiting,
    PreRound,
    Playing,
    PostRound,
}

pub fn run_survival_shrink_phase(state: &mut GameState) {
    let is_playing: bool = state.round.is_some_and(|round| round.phase == RoundPhase::Playing);

    if is_playing {
        state.world.shrink();
    }
}

/// At most one transition per tick.
pub fn run_round_transition_phase(state: &mut GameState, tick: Tick) -> Vec<SimulationEvent> {
    let Some(round) = state.round else {
        return Vec::new();
    };

    let elapsed_ticks: u32 = tick.ticks_since(round.phase_started_at);
    let has_player_minimum: bool = count_participants(state) >= u32::from(state.settings.player_minimum.unwrap_or(0));

    match round.phase {
        RoundPhase::Waiting if has_player_minimum => enter_phase(state, RoundPhase::PreRound, tick),
        RoundPhase::PreRound if !has_player_minimum => enter_phase(state, RoundPhase::Waiting, tick),
        RoundPhase::PreRound if elapsed_ticks == FORCE_SPAWN_ELAPSED_TICKS => spawn::force_spawn_participants(state),
        RoundPhase::PreRound if elapsed_ticks >= ROUND_DELAY_TICKS => enter_phase(state, RoundPhase::Playing, tick),
        RoundPhase::Playing if state.alive_organism_count() <= 1 => end_round(state, tick),
        RoundPhase::PostRound if elapsed_ticks >= ROUND_DELAY_TICKS => start_next_round(state, tick),
        RoundPhase::Waiting | RoundPhase::PreRound | RoundPhase::Playing | RoundPhase::PostRound => Vec::new(),
    }
}

/// Members with the Participant role, alive or not.
pub fn count_participants(state: &GameState) -> u32 {
    let participant_count: usize =
        state.members.values().filter(|member| member.role == MemberRoleKind::Participant).count();

    u32::try_from(participant_count).unwrap_or(u32::MAX)
}

fn enter_phase(state: &mut GameState, phase: RoundPhase, tick: Tick) -> Vec<SimulationEvent> {
    state.round = Some(RoundState {
        phase,
        phase_started_at: tick,
    });

    vec![SimulationEvent::RoundPhaseChanged { phase }]
}

fn end_round(state: &mut GameState, tick: Tick) -> Vec<SimulationEvent> {
    let survivor_id: Option<MemberId> =
        state.members.values().find(|member| member.organism.is_some()).map(|member| member.member_id);
    let mut simulation_events: Vec<SimulationEvent> = enter_phase(state, RoundPhase::PostRound, tick);
    let survivor: Option<&mut Member> = survivor_id.and_then(|member_id| state.members.get_mut(&member_id));

    if let Some(survivor) = survivor {
        survivor.score.wins += 1;
        simulation_events.push(SimulationEvent::RoundWon {
            member_id: survivor.member_id,
        });
    }

    simulation_events
}

fn start_next_round(state: &mut GameState, tick: Tick) -> Vec<SimulationEvent> {
    state.world.restore_initial_bounds();

    let mut simulation_events: Vec<SimulationEvent> = enter_phase(state, RoundPhase::Waiting, tick);
    let spawn_events: Vec<SimulationEvent> = spawn::force_spawn_participants(state);
    simulation_events.extend(spawn_events);

    simulation_events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::world::{WorldBounds, WorldShapeKind};

    const ROUND_START_TICK: Tick = Tick(1000);

    fn create_survival_state(participant_count: u32, phase: RoundPhase) -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Rectangle, 800);
        state.round = Some(RoundState {
            phase,
            phase_started_at: ROUND_START_TICK,
        });

        for index in 0..participant_count {
            let member_id: MemberId = MemberId(index);
            state.members.insert(member_id, test_fixture::create_participant(member_id));
        }

        state
    }

    fn give_organisms(state: &mut GameState, member_ids: &[MemberId]) {
        for (index, member_id) in member_ids.iter().enumerate() {
            let x: i32 = 100 + 100 * i32::try_from(index).unwrap();
            spawn::place_organism(state, *member_id, WorldPoint { x, y: 100 }).unwrap();
        }
    }

    fn get_phase(state: &GameState) -> RoundPhase {
        state.round.unwrap().phase
    }

    fn get_tick_after(elapsed_ticks: u32) -> Tick {
        ROUND_START_TICK.plus(elapsed_ticks)
    }

    #[test]
    fn delays_match_the_designed_tick_counts() {
        assert_eq!(FORCE_SPAWN_ELAPSED_TICKS, 100);
        assert_eq!(ROUND_DELAY_TICKS, 114);
    }

    #[test]
    fn get_countdown_seconds_counts_down_the_delay() {
        let round: RoundState = RoundState {
            phase: RoundPhase::PreRound,
            phase_started_at: ROUND_START_TICK,
        };
        let playing_round: RoundState = RoundState {
            phase: RoundPhase::Playing,
            phase_started_at: ROUND_START_TICK,
        };

        assert_eq!(round.get_countdown_seconds(ROUND_START_TICK), Some(8));
        assert_eq!(round.get_countdown_seconds(get_tick_after(14)), Some(7));
        assert_eq!(round.get_countdown_seconds(get_tick_after(15)), Some(7));
        assert_eq!(round.get_countdown_seconds(get_tick_after(114)), Some(0));
        assert_eq!(playing_round.get_countdown_seconds(ROUND_START_TICK), None);
    }

    #[test]
    fn run_round_transition_phase_starts_the_pre_round_at_the_player_minimum() {
        let mut state: GameState = create_survival_state(1, RoundPhase::Waiting);

        assert_eq!(run_round_transition_phase(&mut state, Tick(1001)), Vec::new());

        state.members.insert(MemberId(1), test_fixture::create_participant(MemberId(1)));

        assert_eq!(
            run_round_transition_phase(&mut state, Tick(1002)),
            vec![SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::PreRound,
            }],
        );
        assert_eq!(
            state.round,
            Some(RoundState {
                phase: RoundPhase::PreRound,
                phase_started_at: Tick(1002),
            }),
        );
    }

    #[test]
    fn run_round_transition_phase_counts_spectators_out() {
        let mut state: GameState = create_survival_state(1, RoundPhase::Waiting);
        let mut spectator: Member = test_fixture::create_participant(MemberId(1));
        spectator.role = MemberRoleKind::Spectator;
        state.members.insert(MemberId(1), spectator);

        assert_eq!(run_round_transition_phase(&mut state, Tick(1001)), Vec::new());
        assert_eq!(get_phase(&state), RoundPhase::Waiting);
    }

    #[test]
    fn run_round_transition_phase_cancels_the_pre_round_below_the_minimum() {
        let mut state: GameState = create_survival_state(1, RoundPhase::PreRound);

        assert_eq!(
            run_round_transition_phase(&mut state, get_tick_after(50)),
            vec![SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::Waiting,
            }],
        );
    }

    #[test]
    fn run_round_transition_phase_force_spawns_during_the_pre_round() {
        let mut state: GameState = create_survival_state(2, RoundPhase::PreRound);

        assert_eq!(run_round_transition_phase(&mut state, get_tick_after(99)), Vec::new());

        let simulation_events: Vec<SimulationEvent> = run_round_transition_phase(&mut state, get_tick_after(100));

        assert_eq!(simulation_events.len(), 2);
        assert_eq!(state.alive_organism_count(), 2);
        assert_eq!(get_phase(&state), RoundPhase::PreRound);
    }

    #[test]
    fn run_round_transition_phase_starts_playing_after_the_delay() {
        let mut state: GameState = create_survival_state(2, RoundPhase::PreRound);

        assert_eq!(run_round_transition_phase(&mut state, get_tick_after(113)), Vec::new());
        assert_eq!(
            run_round_transition_phase(&mut state, get_tick_after(114)),
            vec![SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::Playing,
            }],
        );
    }

    #[test]
    fn run_round_transition_phase_gives_the_sole_survivor_a_win() {
        let mut state: GameState = create_survival_state(3, RoundPhase::Playing);
        give_organisms(&mut state, &[MemberId(0), MemberId(2)]);

        assert_eq!(run_round_transition_phase(&mut state, Tick(1001)), Vec::new());

        state.members.get_mut(&MemberId(0)).unwrap().organism = None;

        assert_eq!(
            run_round_transition_phase(&mut state, Tick(1002)),
            vec![
                SimulationEvent::RoundPhaseChanged {
                    phase: RoundPhase::PostRound,
                },
                SimulationEvent::RoundWon { member_id: MemberId(2) },
            ],
        );
        assert_eq!(state.members[&MemberId(2)].score.wins, 1);
    }

    #[test]
    fn run_round_transition_phase_ends_without_a_winner_when_nobody_survives() {
        let mut state: GameState = create_survival_state(2, RoundPhase::Playing);

        assert_eq!(
            run_round_transition_phase(&mut state, Tick(1001)),
            vec![SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::PostRound,
            }],
        );
        assert!(state.members.values().all(|member| member.score.wins == 0));
    }

    #[test]
    fn run_round_transition_phase_restores_the_world_and_force_spawns_after_the_post_round() {
        let mut state: GameState = create_survival_state(2, RoundPhase::PostRound);
        state.world.shrink();

        assert_eq!(run_round_transition_phase(&mut state, get_tick_after(113)), Vec::new());

        let simulation_events: Vec<SimulationEvent> = run_round_transition_phase(&mut state, get_tick_after(114));

        assert_eq!(
            simulation_events[0],
            SimulationEvent::RoundPhaseChanged {
                phase: RoundPhase::Waiting,
            },
        );
        assert_eq!(simulation_events.len(), 3);
        assert_eq!(state.world.bounds, WorldBounds::from_pixel_size(800, 800));
        assert_eq!(state.alive_organism_count(), 2);
        assert_eq!(get_phase(&state), RoundPhase::Waiting);
    }

    #[test]
    fn run_round_transition_phase_does_nothing_without_rounds() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);

        assert_eq!(run_round_transition_phase(&mut state, Tick(1)), Vec::new());
    }

    #[test]
    fn run_survival_shrink_phase_shrinks_only_while_playing() {
        let mut waiting_state: GameState = create_survival_state(0, RoundPhase::Waiting);
        let mut playing_state: GameState = create_survival_state(0, RoundPhase::Playing);

        run_survival_shrink_phase(&mut waiting_state);
        run_survival_shrink_phase(&mut playing_state);

        assert_eq!(waiting_state.world.bounds, waiting_state.world.initial_bounds);
        assert_ne!(playing_state.world.bounds, playing_state.world.initial_bounds);
    }
}
```

In `shared/src/game/simulation_event.rs`, replace:

```rust
use crate::member::MemberId;
```

with:

```rust
use crate::member::MemberId;
use crate::round::RoundPhase;
```

In `shared/src/game/simulation_event.rs`, replace:

```rust
    EffectApplied {
```

with:

```rust
    RoundPhaseChanged {
        phase: RoundPhase,
    },
    RoundWon {
        member_id: MemberId,
    },
    EffectApplied {
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib round::round`
Expected: no warnings; `test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 179 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 191 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/round/round.rs shared/src/game/simulation_event.rs
git status --short
git commit -m "round transitions and countdown"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 17: Step runs survival shrinking and round transitions

**Files:**
- Modify: `shared/src/game/step.rs`
- Test: inline `mod tests` in `shared/src/game/step.rs`

The tick order is complete: survival shrinking (phase 6) between flight and births, and round transitions with their force spawns (phase 11) after death bookkeeping, so a death in the last tick of a round decides its winner in that tick.

- [ ] **Step 1: Write the failing test**

In `shared/src/game/step.rs`, replace:

```rust
    use super::*;
    use crate::ability::{AbilityPhase, AbilityPressSet, Projectile, SporePhase};
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, WorldPoint};
    use crate::member::{MemberRoleKind, OrganismColorKind, SkinKind, TeamKind};
    use crate::organism::CellOccupancy;
    use crate::world::WorldShapeKind;
```

with:

```rust
    use super::*;
    use crate::ability::{AbilityPhase, AbilityPressSet, Projectile, SporePhase};
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, WorldPoint};
    use crate::member::{MemberRoleKind, OrganismColorKind, SkinKind, TeamKind};
    use crate::organism::CellOccupancy;
    use crate::round::{RoundPhase, RoundState};
    use crate::world::WorldShapeKind;
```

In `shared/src/game/step.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn step_shrinks_a_survival_world_while_playing() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Rectangle, 800);
        state.round = Some(RoundState {
            phase: RoundPhase::Playing,
            phase_started_at: Tick(0),
        });

        step(&mut state, &create_bundle(1, Vec::new())).unwrap();

        assert_eq!(state.world.bounds.width.0, 819_200 - 286);
    }

    #[test]
    fn step_runs_round_transitions_after_death_bookkeeping() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Rectangle, 800);
        state.round = Some(RoundState {
            phase: RoundPhase::Playing,
            phase_started_at: Tick(0),
        });

        for (member_id, x) in [(MemberId(0), 200), (MemberId(1), 400)] {
            state.members.insert(
                member_id,
                test_fixture::create_participant_with_organism(member_id, WorldPoint { x, y: 400 }),
            );
        }

        kill(&mut state, MemberId(0), None);

        let simulation_events: Vec<SimulationEvent> = step(&mut state, &create_bundle(1, Vec::new())).unwrap();

        assert_eq!(
            simulation_events,
            vec![
                SimulationEvent::OrganismDied {
                    member_id: MemberId(0),
                    credited_to: None,
                },
                SimulationEvent::RoundPhaseChanged {
                    phase: RoundPhase::PostRound,
                },
                SimulationEvent::RoundWon { member_id: MemberId(1) },
            ],
        );
    }
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib game::step`
Expected: `test result: FAILED. 18 passed; 2 failed`; the failing tests are `game::step::tests::step_runs_round_transitions_after_death_bookkeeping`, `game::step::tests::step_shrinks_a_survival_world_while_playing`.

- [ ] **Step 3: Implement**

In `shared/src/game/step.rs`, replace:

```rust
use crate::ability::{activation, damage, projectile};
use crate::game::{GameState, InputBundle, MemberEvent, PlayerTickInput, SimulationEvent, Tick};
use crate::member::{Appearance, Member, MemberId, Score};
use crate::organism::Organism;
use crate::organism::{growth, spawn};
```

with:

```rust
use crate::ability::{activation, damage, projectile};
use crate::game::{GameState, InputBundle, MemberEvent, PlayerTickInput, SimulationEvent, Tick};
use crate::member::{Appearance, Member, MemberId, Score};
use crate::organism::Organism;
use crate::organism::{growth, spawn};
use crate::round;
```

In `shared/src/game/step.rs`, replace:

```rust
    apply_cursor_updates(state, &bundle.player_inputs);
    activation::run_timer_expiry_phase(state, bundle.tick);

    let effect_events: Vec<SimulationEvent> =
        activation::run_ability_press_phase(state, &bundle.player_inputs, bundle.tick);
    simulation_events.extend(effect_events);

    projectile::run_flight_phase(state);
    growth::run_birth_phase(state, bundle.tick);
    growth::run_natural_death_phase(state);
    damage::run_damage_phase(state);

    let death_events: Vec<SimulationEvent> = record_deaths(state);
    simulation_events.extend(death_events);

    retighten_cell_occupancies(state);
```

with:

```rust
    apply_cursor_updates(state, &bundle.player_inputs);
    activation::run_timer_expiry_phase(state, bundle.tick);

    let effect_events: Vec<SimulationEvent> =
        activation::run_ability_press_phase(state, &bundle.player_inputs, bundle.tick);
    simulation_events.extend(effect_events);

    projectile::run_flight_phase(state);
    round::run_survival_shrink_phase(state);
    growth::run_birth_phase(state, bundle.tick);
    growth::run_natural_death_phase(state);
    damage::run_damage_phase(state);

    let death_events: Vec<SimulationEvent> = record_deaths(state);
    simulation_events.extend(death_events);

    let round_events: Vec<SimulationEvent> = round::run_round_transition_phase(state, bundle.tick);
    simulation_events.extend(round_events);

    retighten_cell_occupancies(state);
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib game::step`
Expected: no warnings; `test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 173 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 193 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/game/step.rs
git status --short
git commit -m "step runs survival shrinking and round transitions"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 18: Game teams and team balance

**Files:**
- Create: `shared/src/member/team_assignment.rs`
- Modify: `shared/src/member/member.rs`
- Modify: `shared/src/member/mod.rs`
- Test: inline `mod tests` in `shared/src/member/team_assignment.rs` and `shared/src/member/member.rs`

Skirmish teams are the first `team_count` of Red, Blue, Green, Pink. `TeamSizes` counts Participants per team without the requester, and takes pending admissions through `add_member`, so the server checks balance against state plus pending joins. A manual choice is allowed when the member is already on the team or the team is among the smallest; a rejection names the first smaller team in team order, as the original's message did. Automatic assignment picks uniformly among `get_smallest_teams` with the server's own random generator.

- [ ] **Step 1: Write the failing test**

In `shared/src/member/member.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn get_game_teams_takes_the_first_teams_in_order() {
        assert_eq!(get_game_teams(2), vec![TeamKind::Red, TeamKind::Blue]);
        assert_eq!(get_game_teams(4), TEAM_ORDER.to_vec());
    }
```

Create `shared/src/member/team_assignment.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;

    fn create_members(teams: &[TeamKind]) -> BTreeMap<MemberId, Member> {
        let mut members: BTreeMap<MemberId, Member> = BTreeMap::new();

        for (index, team) in teams.iter().enumerate() {
            let member_id: MemberId = MemberId(u32::try_from(index).unwrap());
            let mut member: Member = test_fixture::create_participant(member_id);
            member.team = Some(*team);
            members.insert(member_id, member);
        }

        members
    }

    #[test]
    fn from_members_counts_participants_of_the_game_teams() {
        let mut members: BTreeMap<MemberId, Member> =
            create_members(&[TeamKind::Red, TeamKind::Red, TeamKind::Green, TeamKind::Pink]);
        members.get_mut(&MemberId(1)).unwrap().role = MemberRoleKind::Spectator;

        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 3, None);

        assert_eq!(team_sizes.get_size(TeamKind::Red), Some(1));
        assert_eq!(team_sizes.get_size(TeamKind::Blue), Some(0));
        assert_eq!(team_sizes.get_size(TeamKind::Green), Some(1));
        assert_eq!(team_sizes.get_size(TeamKind::Pink), None);
    }

    #[test]
    fn from_members_leaves_out_the_excluded_member() {
        let members: BTreeMap<MemberId, Member> = create_members(&[TeamKind::Red, TeamKind::Blue]);

        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 2, Some(MemberId(0)));

        assert_eq!(team_sizes.get_size(TeamKind::Red), Some(0));
        assert_eq!(team_sizes.get_size(TeamKind::Blue), Some(1));
    }

    #[test]
    fn add_member_counts_a_pending_member() {
        let mut team_sizes: TeamSizes = TeamSizes::from_members(&BTreeMap::new(), 2, None);
        team_sizes.add_member(TeamKind::Blue);
        team_sizes.add_member(TeamKind::Pink);

        assert_eq!(team_sizes.get_size(TeamKind::Blue), Some(1));
        assert_eq!(team_sizes.get_size(TeamKind::Pink), None);
    }

    #[test]
    fn get_smallest_teams_lists_every_team_of_the_minimum_size() {
        let members: BTreeMap<MemberId, Member> = create_members(&[TeamKind::Blue]);

        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 3, None);

        assert_eq!(team_sizes.get_smallest_teams(), vec![TeamKind::Red, TeamKind::Green]);
    }

    #[test]
    fn check_team_choice_allows_a_smallest_team() {
        let members: BTreeMap<MemberId, Member> = create_members(&[TeamKind::Red]);
        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 3, None);

        assert_eq!(check_team_choice(&team_sizes, None, TeamKind::Blue), Ok(()));
        assert_eq!(check_team_choice(&team_sizes, None, TeamKind::Green), Ok(()));
    }

    #[test]
    fn check_team_choice_names_the_first_smaller_team() {
        let members: BTreeMap<MemberId, Member> =
            create_members(&[TeamKind::Red, TeamKind::Red, TeamKind::Blue, TeamKind::Red]);
        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 3, None);

        assert_eq!(
            check_team_choice(&team_sizes, None, TeamKind::Red),
            Err(TeamChoiceRejectionKind::TeamUnbalanced {
                smaller: TeamKind::Blue,
            }),
        );
        assert_eq!(
            check_team_choice(&team_sizes, None, TeamKind::Blue),
            Err(TeamChoiceRejectionKind::TeamUnbalanced {
                smaller: TeamKind::Green,
            }),
        );
    }

    #[test]
    fn check_team_choice_allows_staying_on_the_current_team() {
        let members: BTreeMap<MemberId, Member> = create_members(&[TeamKind::Red, TeamKind::Red, TeamKind::Red]);
        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 2, Some(MemberId(0)));

        assert_eq!(
            check_team_choice(&team_sizes, Some(TeamKind::Red), TeamKind::Red),
            Ok(())
        );
    }

    #[test]
    fn check_team_choice_rejects_a_team_outside_the_game() {
        let team_sizes: TeamSizes = TeamSizes::from_members(&BTreeMap::new(), 2, None);

        assert_eq!(
            check_team_choice(&team_sizes, None, TeamKind::Green),
            Err(TeamChoiceRejectionKind::TeamNotInGame),
        );
    }
}
```

Replace the whole of `shared/src/member/mod.rs` with:

```rust
pub mod member;
pub mod scoreboard;
pub mod team_assignment;

pub use member::*;
pub use scoreboard::*;
pub use team_assignment::*;
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib member`
Expected: compilation fails; the first error is `` error[E0425]: cannot find value `TEAM_ORDER` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/member/member.rs`, replace:

```rust
use crate::organism::Organism;
```

with:

```rust
use crate::organism::Organism;

pub const TEAM_ORDER: [TeamKind; 4] = [TeamKind::Red, TeamKind::Blue, TeamKind::Green, TeamKind::Pink];
```

In `shared/src/member/member.rs`, insert directly above `/// Both on one team;`:

```rust
/// The first `team_count` teams in team order.
pub fn get_game_teams(team_count: u8) -> Vec<TeamKind> {
    TEAM_ORDER.into_iter().take(usize::from(team_count)).collect()
}

```

Replace the whole of `shared/src/member/team_assignment.rs` with:

```rust
use std::collections::BTreeMap;

use crate::member;
use crate::member::{Member, MemberId, MemberRoleKind, TeamKind};

/// Participants on each team of the game, in team order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamSizes {
    participant_counts: Vec<(TeamKind, u32)>,
}

impl TeamSizes {
    /// `excluded_member_id`, the requester, is not counted.
    pub fn from_members(
        members: &BTreeMap<MemberId, Member>,
        team_count: u8,
        excluded_member_id: Option<MemberId>,
    ) -> TeamSizes {
        let mut team_sizes: TeamSizes = TeamSizes {
            participant_counts: member::get_game_teams(team_count).into_iter().map(|team| (team, 0)).collect(),
        };

        for member in members.values() {
            let Some(team) = member.team else {
                continue;
            };

            let is_counted: bool =
                member.role == MemberRoleKind::Participant && Some(member.member_id) != excluded_member_id;

            if is_counted {
                team_sizes.add_member(team);
            }
        }

        team_sizes
    }

    /// Teams outside the game are ignored.
    pub fn add_member(&mut self, team: TeamKind) {
        let participant_count: Option<&mut u32> = self
            .participant_counts
            .iter_mut()
            .find(|(counted_team, _)| *counted_team == team)
            .map(|(_, participant_count)| participant_count);

        if let Some(participant_count) = participant_count {
            *participant_count += 1;
        }
    }

    /// `None` for a team outside the game.
    pub fn get_size(&self, team: TeamKind) -> Option<u32> {
        self.participant_counts
            .iter()
            .find(|(counted_team, _)| *counted_team == team)
            .map(|(_, participant_count)| *participant_count)
    }

    /// The candidates of an automatic assignment, in team order.
    pub fn get_smallest_teams(&self) -> Vec<TeamKind> {
        let smallest_size: Option<u32> =
            self.participant_counts.iter().map(|(_, participant_count)| *participant_count).min();

        self.participant_counts
            .iter()
            .filter(|(_, participant_count)| Some(*participant_count) == smallest_size)
            .map(|(team, _)| *team)
            .collect()
    }

    /// The first team, in team order, with fewer participants than `team_size`.
    fn find_smaller_team(&self, team_size: u32) -> Option<TeamKind> {
        self.participant_counts
            .iter()
            .find(|(_, participant_count)| *participant_count < team_size)
            .map(|(team, _)| *team)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamChoiceRejectionKind {
    TeamNotInGame,
    TeamUnbalanced { smaller: TeamKind },
}

/// Allowed when the member is already on `requested`, or when `requested` is among the smallest teams.
pub fn check_team_choice(
    team_sizes: &TeamSizes,
    current_team: Option<TeamKind>,
    requested: TeamKind,
) -> Result<(), TeamChoiceRejectionKind> {
    let Some(requested_size) = team_sizes.get_size(requested) else {
        return Err(TeamChoiceRejectionKind::TeamNotInGame);
    };

    if current_team == Some(requested) {
        return Ok(());
    }

    match team_sizes.find_smaller_team(requested_size) {
        Some(smaller) => Err(TeamChoiceRejectionKind::TeamUnbalanced { smaller }),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;

    fn create_members(teams: &[TeamKind]) -> BTreeMap<MemberId, Member> {
        let mut members: BTreeMap<MemberId, Member> = BTreeMap::new();

        for (index, team) in teams.iter().enumerate() {
            let member_id: MemberId = MemberId(u32::try_from(index).unwrap());
            let mut member: Member = test_fixture::create_participant(member_id);
            member.team = Some(*team);
            members.insert(member_id, member);
        }

        members
    }

    #[test]
    fn from_members_counts_participants_of_the_game_teams() {
        let mut members: BTreeMap<MemberId, Member> =
            create_members(&[TeamKind::Red, TeamKind::Red, TeamKind::Green, TeamKind::Pink]);
        members.get_mut(&MemberId(1)).unwrap().role = MemberRoleKind::Spectator;

        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 3, None);

        assert_eq!(team_sizes.get_size(TeamKind::Red), Some(1));
        assert_eq!(team_sizes.get_size(TeamKind::Blue), Some(0));
        assert_eq!(team_sizes.get_size(TeamKind::Green), Some(1));
        assert_eq!(team_sizes.get_size(TeamKind::Pink), None);
    }

    #[test]
    fn from_members_leaves_out_the_excluded_member() {
        let members: BTreeMap<MemberId, Member> = create_members(&[TeamKind::Red, TeamKind::Blue]);

        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 2, Some(MemberId(0)));

        assert_eq!(team_sizes.get_size(TeamKind::Red), Some(0));
        assert_eq!(team_sizes.get_size(TeamKind::Blue), Some(1));
    }

    #[test]
    fn add_member_counts_a_pending_member() {
        let mut team_sizes: TeamSizes = TeamSizes::from_members(&BTreeMap::new(), 2, None);
        team_sizes.add_member(TeamKind::Blue);
        team_sizes.add_member(TeamKind::Pink);

        assert_eq!(team_sizes.get_size(TeamKind::Blue), Some(1));
        assert_eq!(team_sizes.get_size(TeamKind::Pink), None);
    }

    #[test]
    fn get_smallest_teams_lists_every_team_of_the_minimum_size() {
        let members: BTreeMap<MemberId, Member> = create_members(&[TeamKind::Blue]);

        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 3, None);

        assert_eq!(team_sizes.get_smallest_teams(), vec![TeamKind::Red, TeamKind::Green]);
    }

    #[test]
    fn check_team_choice_allows_a_smallest_team() {
        let members: BTreeMap<MemberId, Member> = create_members(&[TeamKind::Red]);
        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 3, None);

        assert_eq!(check_team_choice(&team_sizes, None, TeamKind::Blue), Ok(()));
        assert_eq!(check_team_choice(&team_sizes, None, TeamKind::Green), Ok(()));
    }

    #[test]
    fn check_team_choice_names_the_first_smaller_team() {
        let members: BTreeMap<MemberId, Member> =
            create_members(&[TeamKind::Red, TeamKind::Red, TeamKind::Blue, TeamKind::Red]);
        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 3, None);

        assert_eq!(
            check_team_choice(&team_sizes, None, TeamKind::Red),
            Err(TeamChoiceRejectionKind::TeamUnbalanced {
                smaller: TeamKind::Blue,
            }),
        );
        assert_eq!(
            check_team_choice(&team_sizes, None, TeamKind::Blue),
            Err(TeamChoiceRejectionKind::TeamUnbalanced {
                smaller: TeamKind::Green,
            }),
        );
    }

    #[test]
    fn check_team_choice_allows_staying_on_the_current_team() {
        let members: BTreeMap<MemberId, Member> = create_members(&[TeamKind::Red, TeamKind::Red, TeamKind::Red]);
        let team_sizes: TeamSizes = TeamSizes::from_members(&members, 2, Some(MemberId(0)));

        assert_eq!(
            check_team_choice(&team_sizes, Some(TeamKind::Red), TeamKind::Red),
            Ok(())
        );
    }

    #[test]
    fn check_team_choice_rejects_a_team_outside_the_game() {
        let team_sizes: TeamSizes = TeamSizes::from_members(&BTreeMap::new(), 2, None);

        assert_eq!(
            check_team_choice(&team_sizes, None, TeamKind::Green),
            Err(TeamChoiceRejectionKind::TeamNotInGame),
        );
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib member`
Expected: no warnings; `test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 174 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 202 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/member/team_assignment.rs shared/src/member/member.rs shared/src/member/mod.rs
git status --short
git commit -m "game teams and team balance"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 19: Leaderboard rows

**Files:**
- Modify: `shared/src/member/scoreboard.rs`
- Test: inline `mod tests` in `shared/src/member/scoreboard.rs`

The leaderboard is derived, never stored. Free for all: Participants by kills descending, deaths ascending, then id. Survival: kills descending, wins descending, then id. Both cut to `leaderboard_length`. Skirmish: one row per game team in team order, summing its current members, never sorted or cut. Pure Spectators are not on the board. The K:D text is computed from integers, rounding half up to at most two decimals as the original's `round(x * 100) / 100`.

- [ ] **Step 1: Write the failing test**

Append to the end of `shared/src/member/scoreboard.rs` (after a blank line):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::world::WorldShapeKind;

    fn create_state_with_scores(mode: GameModeKind, scores: &[(u32, u32, u32)]) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);

        for (index, (kills, deaths, wins)) in scores.iter().enumerate() {
            let member_id: MemberId = MemberId(u32::try_from(index).unwrap());
            let mut member: Member = test_fixture::create_participant(member_id);
            member.score = Score {
                kills: *kills,
                deaths: *deaths,
                wins: *wins,
            };
            state.members.insert(member_id, member);
        }

        state
    }

    fn get_member_ids(leaderboard_rows: &[LeaderboardRow]) -> Vec<MemberId> {
        leaderboard_rows
            .iter()
            .filter_map(|leaderboard_row| match &leaderboard_row.subject {
                LeaderboardSubjectKind::Member { member_id, .. } => Some(*member_id),
                LeaderboardSubjectKind::Team { .. } => None,
            })
            .collect()
    }

    #[test]
    fn get_leaderboard_rows_orders_free_for_all_by_kills_then_deaths_then_id() {
        let state: GameState =
            create_state_with_scores(GameModeKind::FreeForAll, &[(1, 0, 0), (3, 2, 0), (3, 1, 0), (1, 0, 0)]);

        assert_eq!(
            get_member_ids(&get_leaderboard_rows(&state)),
            vec![MemberId(2), MemberId(1), MemberId(0), MemberId(3)],
        );
    }

    #[test]
    fn get_leaderboard_rows_orders_survival_by_kills_then_wins_then_id() {
        let state: GameState =
            create_state_with_scores(GameModeKind::Survival, &[(2, 0, 1), (2, 5, 3), (4, 0, 0), (2, 0, 1)]);

        assert_eq!(
            get_member_ids(&get_leaderboard_rows(&state)),
            vec![MemberId(2), MemberId(1), MemberId(0), MemberId(3)],
        );
    }

    #[test]
    fn get_leaderboard_rows_cuts_player_rows_to_the_leaderboard_length() {
        let mut state: GameState =
            create_state_with_scores(GameModeKind::FreeForAll, &[(0, 0, 0), (5, 0, 0), (2, 0, 0)]);
        state.settings.leaderboard_length = 2;

        assert_eq!(
            get_member_ids(&get_leaderboard_rows(&state)),
            vec![MemberId(1), MemberId(2)],
        );
    }

    #[test]
    fn get_leaderboard_rows_leaves_out_pure_spectators() {
        let mut state: GameState = create_state_with_scores(GameModeKind::FreeForAll, &[(0, 0, 0), (0, 0, 0)]);
        state.members.get_mut(&MemberId(0)).unwrap().role = MemberRoleKind::Spectator;

        assert_eq!(get_member_ids(&get_leaderboard_rows(&state)), vec![MemberId(1)]);
    }

    #[test]
    fn get_leaderboard_rows_sums_teams_in_team_order() {
        let mut state: GameState = create_state_with_scores(GameModeKind::Skirmish, &[(1, 2, 0), (4, 0, 0), (2, 3, 0)]);
        state.settings.leaderboard_length = 1;

        for (member_id, team) in [
            (MemberId(0), TeamKind::Blue),
            (MemberId(1), TeamKind::Red),
            (MemberId(2), TeamKind::Blue),
        ] {
            state.members.get_mut(&member_id).unwrap().team = Some(team);
        }

        assert_eq!(
            get_leaderboard_rows(&state),
            vec![
                LeaderboardRow {
                    subject: LeaderboardSubjectKind::Team { team: TeamKind::Red },
                    score: Score {
                        kills: 4,
                        deaths: 0,
                        wins: 0,
                    },
                },
                LeaderboardRow {
                    subject: LeaderboardSubjectKind::Team { team: TeamKind::Blue },
                    score: Score {
                        kills: 3,
                        deaths: 5,
                        wins: 0,
                    },
                },
            ],
        );
    }

    #[test]
    fn get_leaderboard_columns_follow_the_mode() {
        assert_eq!(
            get_leaderboard_columns(GameModeKind::Survival),
            vec![
                LeaderboardColumnKind::Player,
                LeaderboardColumnKind::Wins,
                LeaderboardColumnKind::Kills,
            ],
        );
        assert_eq!(
            get_leaderboard_columns(GameModeKind::Skirmish)[0],
            LeaderboardColumnKind::Team
        );
    }

    #[test]
    fn format_kill_death_ratio_handles_zero_deaths() {
        assert_eq!(format_kill_death_ratio(0, 0), "0");
        assert_eq!(format_kill_death_ratio(3, 0), "∞");
    }

    #[test]
    fn format_kill_death_ratio_drops_trailing_zeros() {
        assert_eq!(format_kill_death_ratio(2, 1), "2");
        assert_eq!(format_kill_death_ratio(3, 2), "1.5");
        assert_eq!(format_kill_death_ratio(1, 3), "0.33");
        assert_eq!(format_kill_death_ratio(2, 3), "0.67");
        assert_eq!(format_kill_death_ratio(0, 4), "0");
    }

    #[test]
    fn format_kill_death_ratio_rounds_halves_up() {
        assert_eq!(format_kill_death_ratio(1, 8), "0.13");
        assert_eq!(format_kill_death_ratio(1, 200), "0.01");
    }
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib member::scoreboard`
Expected: compilation fails; the first error is `` error[E0425]: cannot find type `GameModeKind` in this scope ``.

- [ ] **Step 3: Implement**

Replace the whole of `shared/src/member/scoreboard.rs` with:

```rust
use std::cmp::Reverse;

use crate::game::{GameModeKind, GameState};
use crate::member;
use crate::member::{Member, MemberId, MemberRoleKind, TeamKind};

const INFINITE_RATIO_TEXT: &str = "∞";
const ZERO_RATIO_TEXT: &str = "0";

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

    fn plus(self, other: Score) -> Score {
        Score {
            kills: self.kills + other.kills,
            deaths: self.deaths + other.deaths,
            wins: self.wins + other.wins,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LeaderboardSubjectKind {
    Member { member_id: MemberId, screen_name: String },
    Team { team: TeamKind },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaderboardRow {
    pub subject: LeaderboardSubjectKind,
    /// A team row sums its current members.
    pub score: Score,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaderboardColumnKind {
    Player,
    Team,
    Kills,
    Deaths,
    KillDeathRatio,
    Wins,
}

pub fn get_leaderboard_columns(mode: GameModeKind) -> Vec<LeaderboardColumnKind> {
    match mode {
        GameModeKind::FreeForAll => vec![
            LeaderboardColumnKind::Player,
            LeaderboardColumnKind::Kills,
            LeaderboardColumnKind::Deaths,
            LeaderboardColumnKind::KillDeathRatio,
        ],
        GameModeKind::Skirmish => vec![
            LeaderboardColumnKind::Team,
            LeaderboardColumnKind::Kills,
            LeaderboardColumnKind::Deaths,
            LeaderboardColumnKind::KillDeathRatio,
        ],
        GameModeKind::Survival => vec![
            LeaderboardColumnKind::Player,
            LeaderboardColumnKind::Wins,
            LeaderboardColumnKind::Kills,
        ],
    }
}

/// Player rows are cut to the leaderboard length; team rows are all shown, in team order.
pub fn get_leaderboard_rows(state: &GameState) -> Vec<LeaderboardRow> {
    match state.settings.mode {
        GameModeKind::FreeForAll => {
            let mut participants: Vec<&Member> = get_participants(state);
            participants.sort_by_key(|participant| {
                (
                    Reverse(participant.score.kills),
                    participant.score.deaths,
                    participant.member_id,
                )
            });

            get_member_rows(&participants, state.settings.leaderboard_length)
        }
        GameModeKind::Survival => {
            let mut participants: Vec<&Member> = get_participants(state);
            participants.sort_by_key(|participant| {
                (
                    Reverse(participant.score.kills),
                    Reverse(participant.score.wins),
                    participant.member_id,
                )
            });

            get_member_rows(&participants, state.settings.leaderboard_length)
        }
        GameModeKind::Skirmish => get_team_rows(state),
    }
}

/// "∞" for kills without deaths, "0" for neither, else rounded half up to at most two decimals.
pub fn format_kill_death_ratio(kills: u32, deaths: u32) -> String {
    if deaths == 0 {
        let ratio_text: &str = if kills == 0 {
            ZERO_RATIO_TEXT
        } else {
            INFINITE_RATIO_TEXT
        };

        return String::from(ratio_text);
    }

    let hundredths: u64 = (u64::from(kills) * 200 + u64::from(deaths)) / (2 * u64::from(deaths));
    let whole: u64 = hundredths / 100;
    let fraction: u64 = hundredths % 100;

    match fraction {
        0 => format!("{whole}"),
        tenths_only if tenths_only % 10 == 0 => format!("{whole}.{}", tenths_only / 10),
        _ => format!("{whole}.{fraction:02}"),
    }
}

/// Pure Spectators are not on the board; dead Participants are.
fn get_participants(state: &GameState) -> Vec<&Member> {
    state.members.values().filter(|member| member.role == MemberRoleKind::Participant).collect()
}

fn get_member_rows(participants: &[&Member], leaderboard_length: u8) -> Vec<LeaderboardRow> {
    participants
        .iter()
        .take(usize::from(leaderboard_length))
        .map(|participant| LeaderboardRow {
            subject: LeaderboardSubjectKind::Member {
                member_id: participant.member_id,
                screen_name: participant.screen_name.clone(),
            },
            score: participant.score,
        })
        .collect()
}

fn get_team_rows(state: &GameState) -> Vec<LeaderboardRow> {
    let teams: Vec<TeamKind> = member::get_game_teams(state.settings.team_count.unwrap_or(0));

    teams
        .into_iter()
        .map(|team| {
            let team_score: Score = state
                .members
                .values()
                .filter(|member| member.team == Some(team))
                .fold(Score::zero(), |team_score, member| team_score.plus(member.score));

            LeaderboardRow {
                subject: LeaderboardSubjectKind::Team { team },
                score: team_score,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::world::WorldShapeKind;

    fn create_state_with_scores(mode: GameModeKind, scores: &[(u32, u32, u32)]) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);

        for (index, (kills, deaths, wins)) in scores.iter().enumerate() {
            let member_id: MemberId = MemberId(u32::try_from(index).unwrap());
            let mut member: Member = test_fixture::create_participant(member_id);
            member.score = Score {
                kills: *kills,
                deaths: *deaths,
                wins: *wins,
            };
            state.members.insert(member_id, member);
        }

        state
    }

    fn get_member_ids(leaderboard_rows: &[LeaderboardRow]) -> Vec<MemberId> {
        leaderboard_rows
            .iter()
            .filter_map(|leaderboard_row| match &leaderboard_row.subject {
                LeaderboardSubjectKind::Member { member_id, .. } => Some(*member_id),
                LeaderboardSubjectKind::Team { .. } => None,
            })
            .collect()
    }

    #[test]
    fn get_leaderboard_rows_orders_free_for_all_by_kills_then_deaths_then_id() {
        let state: GameState =
            create_state_with_scores(GameModeKind::FreeForAll, &[(1, 0, 0), (3, 2, 0), (3, 1, 0), (1, 0, 0)]);

        assert_eq!(
            get_member_ids(&get_leaderboard_rows(&state)),
            vec![MemberId(2), MemberId(1), MemberId(0), MemberId(3)],
        );
    }

    #[test]
    fn get_leaderboard_rows_orders_survival_by_kills_then_wins_then_id() {
        let state: GameState =
            create_state_with_scores(GameModeKind::Survival, &[(2, 0, 1), (2, 5, 3), (4, 0, 0), (2, 0, 1)]);

        assert_eq!(
            get_member_ids(&get_leaderboard_rows(&state)),
            vec![MemberId(2), MemberId(1), MemberId(0), MemberId(3)],
        );
    }

    #[test]
    fn get_leaderboard_rows_cuts_player_rows_to_the_leaderboard_length() {
        let mut state: GameState =
            create_state_with_scores(GameModeKind::FreeForAll, &[(0, 0, 0), (5, 0, 0), (2, 0, 0)]);
        state.settings.leaderboard_length = 2;

        assert_eq!(
            get_member_ids(&get_leaderboard_rows(&state)),
            vec![MemberId(1), MemberId(2)],
        );
    }

    #[test]
    fn get_leaderboard_rows_leaves_out_pure_spectators() {
        let mut state: GameState = create_state_with_scores(GameModeKind::FreeForAll, &[(0, 0, 0), (0, 0, 0)]);
        state.members.get_mut(&MemberId(0)).unwrap().role = MemberRoleKind::Spectator;

        assert_eq!(get_member_ids(&get_leaderboard_rows(&state)), vec![MemberId(1)]);
    }

    #[test]
    fn get_leaderboard_rows_sums_teams_in_team_order() {
        let mut state: GameState = create_state_with_scores(GameModeKind::Skirmish, &[(1, 2, 0), (4, 0, 0), (2, 3, 0)]);
        state.settings.leaderboard_length = 1;

        for (member_id, team) in [
            (MemberId(0), TeamKind::Blue),
            (MemberId(1), TeamKind::Red),
            (MemberId(2), TeamKind::Blue),
        ] {
            state.members.get_mut(&member_id).unwrap().team = Some(team);
        }

        assert_eq!(
            get_leaderboard_rows(&state),
            vec![
                LeaderboardRow {
                    subject: LeaderboardSubjectKind::Team { team: TeamKind::Red },
                    score: Score {
                        kills: 4,
                        deaths: 0,
                        wins: 0,
                    },
                },
                LeaderboardRow {
                    subject: LeaderboardSubjectKind::Team { team: TeamKind::Blue },
                    score: Score {
                        kills: 3,
                        deaths: 5,
                        wins: 0,
                    },
                },
            ],
        );
    }

    #[test]
    fn get_leaderboard_columns_follow_the_mode() {
        assert_eq!(
            get_leaderboard_columns(GameModeKind::Survival),
            vec![
                LeaderboardColumnKind::Player,
                LeaderboardColumnKind::Wins,
                LeaderboardColumnKind::Kills,
            ],
        );
        assert_eq!(
            get_leaderboard_columns(GameModeKind::Skirmish)[0],
            LeaderboardColumnKind::Team
        );
    }

    #[test]
    fn format_kill_death_ratio_handles_zero_deaths() {
        assert_eq!(format_kill_death_ratio(0, 0), "0");
        assert_eq!(format_kill_death_ratio(3, 0), "∞");
    }

    #[test]
    fn format_kill_death_ratio_drops_trailing_zeros() {
        assert_eq!(format_kill_death_ratio(2, 1), "2");
        assert_eq!(format_kill_death_ratio(3, 2), "1.5");
        assert_eq!(format_kill_death_ratio(1, 3), "0.33");
        assert_eq!(format_kill_death_ratio(2, 3), "0.67");
        assert_eq!(format_kill_death_ratio(0, 4), "0");
    }

    #[test]
    fn format_kill_death_ratio_rounds_halves_up() {
        assert_eq!(format_kill_death_ratio(1, 8), "0.13");
        assert_eq!(format_kill_death_ratio(1, 200), "0.01");
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib member::scoreboard`
Expected: no warnings; `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 202 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 211 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/member/scoreboard.rs
git status --short
git commit -m "leaderboard rows"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 20: Ability icons

**Files:**
- Create: `shared/src/ability/ability_icon.rs`
- Modify: `shared/src/ability/mod.rs`
- Test: inline `mod tests` in `shared/src/ability/ability_icon.rs`

The original drew each HUD icon with a pixel pen (`public/js/ui/items.js`, `itemize` with width 1): one pixel one up and one left of the slot centre, then one pixel after every step. The paths are its arrays with 0, 1, 2, 3 written as L, U, D, R; secrete reuses the spore path, as the original's identical arrays. The test pins each icon's distinct pixel count and bounding box, measured from the original arrays.

- [ ] **Step 1: Write the failing test**

Create `shared/src/ability/ability_icon.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    const ICONS: [AbilityIconKind; 8] = [
        AbilityIconKind::Extend,
        AbilityIconKind::Compress,
        AbilityIconKind::Immortality,
        AbilityIconKind::Freeze,
        AbilityIconKind::Neutralize,
        AbilityIconKind::Toxin,
        AbilityIconKind::Spore,
        AbilityIconKind::Secrete,
    ];

    /// Distinct pixels and bounding box (minimum x, maximum x, minimum y, maximum y) of the original's drawings.
    fn get_original_icon_shape(icon: AbilityIconKind) -> (usize, (i32, i32, i32, i32)) {
        match icon {
            AbilityIconKind::Extend => (197, (-15, 13, -7, 5)),
            AbilityIconKind::Compress => (194, (-14, 13, -7, 5)),
            AbilityIconKind::Immortality => (173, (-15, 13, -8, 6)),
            AbilityIconKind::Freeze => (220, (-12, 10, -12, 10)),
            AbilityIconKind::Neutralize => (156, (-11, 12, -13, 11)),
            AbilityIconKind::Toxin => (498, (-13, 15, -14, 12)),
            AbilityIconKind::Spore | AbilityIconKind::Secrete => (189, (-10, 8, -10, 8)),
        }
    }

    #[test]
    fn pen_paths_hold_only_step_letters() {
        for icon in ICONS {
            assert!(icon.pen_path().chars().all(|letter| PenStepKind::from_letter(letter).is_some()));
        }
    }

    #[test]
    fn get_icon_pixels_draws_one_pixel_per_step_from_up_left_of_the_centre() {
        let icon_pixels: Vec<IconPixel> = get_icon_pixels(AbilityIconKind::Freeze);

        assert_eq!(icon_pixels.len(), 267);
        assert_eq!(icon_pixels[0], IconPixel { x: -1, y: -1 });
        assert_eq!(icon_pixels[1], IconPixel { x: 0, y: -1 });
    }

    #[test]
    fn get_icon_pixels_reproduces_the_original_drawings() {
        for icon in ICONS {
            let icon_pixels: Vec<IconPixel> = get_icon_pixels(icon);
            let distinct_pixels: BTreeSet<(i32, i32)> =
                icon_pixels.iter().map(|icon_pixel| (icon_pixel.x, icon_pixel.y)).collect();
            let bounding_box: (i32, i32, i32, i32) = (
                icon_pixels.iter().map(|icon_pixel| icon_pixel.x).min().unwrap(),
                icon_pixels.iter().map(|icon_pixel| icon_pixel.x).max().unwrap(),
                icon_pixels.iter().map(|icon_pixel| icon_pixel.y).min().unwrap(),
                icon_pixels.iter().map(|icon_pixel| icon_pixel.y).max().unwrap(),
            );

            assert_eq!((distinct_pixels.len(), bounding_box), get_original_icon_shape(icon));
        }
    }
}
```

Replace the whole of `shared/src/ability/mod.rs` with:

```rust
pub mod ability_constants;
pub mod ability_icon;
pub mod ability_model;
pub mod activation;
pub mod damage;
pub mod projectile;

pub use ability_constants::*;
pub use ability_icon::*;
pub use ability_model::*;
pub use activation::*;
pub use damage::*;
pub use projectile::*;
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib ability::ability_icon`
Expected: compilation fails; the first error is `` error[E0425]: cannot find type `AbilityIconKind` in this scope ``.

- [ ] **Step 3: Implement**

Replace the whole of `shared/src/ability/ability_icon.rs` with:

```rust
/// Steps of the original's pixel pen: L left, U up, R right, D down.
const EXTEND_PEN_PATH: &str = concat!(
    "DDLLLLLLLLDDDDLUUUUURULURULURULURULDLDDDDDDDDDDULUUUUUUUUDLDDDDDDULUUUUDLDDRRRRRRRUUURDDDRUUURDD",
    "DRUUURDDDRUUURDRUDDDDRUUUURDDDDRUUUURDDDDRUUUURDDDDRDDDDRUUUUULURULURULURULURDRDDDDDDDDDDURUUUUU",
    "UUUDRDDDDDDURUUUUDRDD",
);
const COMPRESS_PEN_PATH: &str = concat!(
    "LLLLLLLLLLLLLUUDDDDRUUUURDDDDRUUUURDDDDRUUUURDDDDRDDDDRUUUUULURULURULURULURDRDDDDDDDDDDURUUUUUUU",
    "UDRDDDDDDURUUUUDRDDRDUUUURUDDDDDDRDUUUUUUUURUDDDDDDDDDDRDRUULURULURULURULURULURULRDDDDRDDDDRUUUU",
    "RDDDDRUUUURDDDDRUUUURDDDDRUUUU",
);
const IMMORTALITY_PEN_PATH: &str = concat!(
    "DDLUDDLUDDLUDDLULDDRLLDLLURRULLLDLURULLDULURLLURLLURULURULRRULRRULRRRULLRURRUDDRUURDDRURDDLRRUDR",
    "DLRRDLRDRURDURULRURDUURURDDLRURUURDDRUUDRRDLDRRUDDRUDRDLRRDLDRDLDRLDLUDDLUDDLUDDLUUDLDDLUULDDULU",
    "ULDDULUULDUULDUU",
);
const FREEZE_PEN_PATH: &str = concat!(
    "RRLLLLLULRRRRRLULLLLLULRRRRRLULLLLLULLLRRRRRRRLULLLLLURRRRULLLRURRRRRULLLRRRURDRURDRUDDRURDRUDDR",
    "URDRDLRRDLDRRDLDRDRDLDRDLDRLDLDRDLLLLLLLLULRRRRRLULLLLLULRRRRRLULLLLLRDRDRDRDRDRRRRRRDLLLLLRRRRD",
    "LLLDRRLLLLLDRRRLLLDLULDLULDUULDLULDUULDLULURLLURULLURULURLLURULURULRURULUR",
);
const NEUTRALIZE_PEN_PATH: &str = concat!(
    "RRUUUUURRRRRRUUUURUULULRDRDRDRDRDRDULULULLLDLRRRDLLLLDRRRLLLLLUDLDLRRRRRRLDLLLLLLDLRRRRRRRRLDLLL",
    "LLLLLDLRRRRRRRRLDLLLLLLLLDLRRRRRRRRLDLLLLLLLLDLRRRRRRRRLDLLLLLLLLDLRRRRRRRRLDLLLLLLLRDRRRRRLDLDL",
    "ULLDLDLDDLU",
);
const TOXIN_PEN_PATH: &str = concat!(
    "LLLLLLDDDDDDDDDDDLDLLLULRRRRRULLLLLLRRRRRRRRRRRRRRRRRRRRDLLLLLLLLDRRRRRRRRLDLLLLLLUUUURRRRRRRLLL",
    "LLLLLLLLLLLLLLLLLURRRRRRRRRRRRRRRRRRRULLLLLLLLLLLLLLLLLLLRURRRRRRRRRRRRRRRRRRRURLLLLLLLLLLLLLLLL",
    "LLLLRURRRRRRRRRRRRRRRRRRRRURRRLLLLLLLLLLLLLLLLLLLLLLRRURRRRRRRRRRRRRRRRRRRRRRURLLLLLLLLLLLLLLLLL",
    "LLLLLURRRRRRRRRRRRRRRRRRRRRRULLLLLLLLLLLLLLLLLLLLLLURRRRRRRRRRRRRRRRRRRRRRLULLLLLLLLLLLLLLLLLLLL",
    "LULRRRRRRRRRRRRRRRRRRRRRRLLULLLLLLLLLLLLLLLLLLLLLULRRRRRRRRRRRRRRRLULLLLLLLLLLLLLLLURRRRRRRRRRRR",
    "RRRRURLLLLLLLLLLLLLLLLLURRRRRRRRRRRRRRRRRULLLLLLLLDLLLLULLLLRRRRDRRRRUURRRRRRRRLLULLLLL",
);
const SPORE_PEN_PATH: &str = concat!(
    "ULUULULUUUURURRRRDRRRLLLLLLLLLLDLRRRLLDLLDRLLDRDLDLDRDLDRDLRDRDLDRRDLRRDLRRRDLLRRRDRURDRURDURRRU",
    "LLRRRULRRULRRULURURULURULURLULURULLURLLURLLLULLLDDDDUUUURRRDDDDLDLDDRRURURRRRDDDLLLLRRRRDDDLLLLU",
    "LULLDDRDRDDDDLLLUUUUDDDDLLLUUUURURUULLDLDLLLLUUURRRRLLLLUUURRRRDRDRRDDRRUUL",
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbilityIconKind {
    Extend,
    Compress,
    Immortality,
    Freeze,
    Neutralize,
    Toxin,
    Spore,
    Secrete,
}

impl AbilityIconKind {
    fn pen_path(self) -> &'static str {
        match self {
            AbilityIconKind::Extend => EXTEND_PEN_PATH,
            AbilityIconKind::Compress => COMPRESS_PEN_PATH,
            AbilityIconKind::Immortality => IMMORTALITY_PEN_PATH,
            AbilityIconKind::Freeze => FREEZE_PEN_PATH,
            AbilityIconKind::Neutralize => NEUTRALIZE_PEN_PATH,
            AbilityIconKind::Toxin => TOXIN_PEN_PATH,
            AbilityIconKind::Spore | AbilityIconKind::Secrete => SPORE_PEN_PATH,
        }
    }
}

/// One pixel of an icon, offset from the centre of its ability slot in CSS px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconPixel {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PenStepKind {
    Left,
    Up,
    Right,
    Down,
}

impl PenStepKind {
    fn from_letter(letter: char) -> Option<PenStepKind> {
        match letter {
            'L' => Some(PenStepKind::Left),
            'U' => Some(PenStepKind::Up),
            'R' => Some(PenStepKind::Right),
            'D' => Some(PenStepKind::Down),
            _ => None,
        }
    }

    fn offset(self) -> (i32, i32) {
        match self {
            PenStepKind::Left => (-1, 0),
            PenStepKind::Up => (0, -1),
            PenStepKind::Right => (1, 0),
            PenStepKind::Down => (0, 1),
        }
    }
}

/// In drawing order, repeats included; the pen starts one pixel up and left of the slot centre (faithful).
pub fn get_icon_pixels(icon: AbilityIconKind) -> Vec<IconPixel> {
    let mut pen: IconPixel = IconPixel { x: -1, y: -1 };
    let mut icon_pixels: Vec<IconPixel> = vec![pen];

    for pen_step in icon.pen_path().chars().filter_map(PenStepKind::from_letter) {
        let (offset_x, offset_y): (i32, i32) = pen_step.offset();
        pen = IconPixel {
            x: pen.x + offset_x,
            y: pen.y + offset_y,
        };
        icon_pixels.push(pen);
    }

    icon_pixels
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    const ICONS: [AbilityIconKind; 8] = [
        AbilityIconKind::Extend,
        AbilityIconKind::Compress,
        AbilityIconKind::Immortality,
        AbilityIconKind::Freeze,
        AbilityIconKind::Neutralize,
        AbilityIconKind::Toxin,
        AbilityIconKind::Spore,
        AbilityIconKind::Secrete,
    ];

    /// Distinct pixels and bounding box (minimum x, maximum x, minimum y, maximum y) of the original's drawings.
    fn get_original_icon_shape(icon: AbilityIconKind) -> (usize, (i32, i32, i32, i32)) {
        match icon {
            AbilityIconKind::Extend => (197, (-15, 13, -7, 5)),
            AbilityIconKind::Compress => (194, (-14, 13, -7, 5)),
            AbilityIconKind::Immortality => (173, (-15, 13, -8, 6)),
            AbilityIconKind::Freeze => (220, (-12, 10, -12, 10)),
            AbilityIconKind::Neutralize => (156, (-11, 12, -13, 11)),
            AbilityIconKind::Toxin => (498, (-13, 15, -14, 12)),
            AbilityIconKind::Spore | AbilityIconKind::Secrete => (189, (-10, 8, -10, 8)),
        }
    }

    #[test]
    fn pen_paths_hold_only_step_letters() {
        for icon in ICONS {
            assert!(icon.pen_path().chars().all(|letter| PenStepKind::from_letter(letter).is_some()));
        }
    }

    #[test]
    fn get_icon_pixels_draws_one_pixel_per_step_from_up_left_of_the_centre() {
        let icon_pixels: Vec<IconPixel> = get_icon_pixels(AbilityIconKind::Freeze);

        assert_eq!(icon_pixels.len(), 267);
        assert_eq!(icon_pixels[0], IconPixel { x: -1, y: -1 });
        assert_eq!(icon_pixels[1], IconPixel { x: 0, y: -1 });
    }

    #[test]
    fn get_icon_pixels_reproduces_the_original_drawings() {
        for icon in ICONS {
            let icon_pixels: Vec<IconPixel> = get_icon_pixels(icon);
            let distinct_pixels: BTreeSet<(i32, i32)> =
                icon_pixels.iter().map(|icon_pixel| (icon_pixel.x, icon_pixel.y)).collect();
            let bounding_box: (i32, i32, i32, i32) = (
                icon_pixels.iter().map(|icon_pixel| icon_pixel.x).min().unwrap(),
                icon_pixels.iter().map(|icon_pixel| icon_pixel.x).max().unwrap(),
                icon_pixels.iter().map(|icon_pixel| icon_pixel.y).min().unwrap(),
                icon_pixels.iter().map(|icon_pixel| icon_pixel.y).max().unwrap(),
            );

            assert_eq!((distinct_pixels.len(), bounding_box), get_original_icon_shape(icon));
        }
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib ability::ability_icon`
Expected: no warnings; `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 211 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 214 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/ability/ability_icon.rs shared/src/ability/mod.rs
git status --short
git commit -m "ability icons"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 21: Geometry wire types

**Files:**
- Create: `shared/src/protocol/geometry_serial.rs`
- Create: `shared/src/protocol/mod.rs`
- Modify: `shared/src/lib.rs`
- Test: inline `mod tests` in `shared/src/protocol/geometry_serial.rs`

The state checksum covers exactly the wire encoding of the state (`GameStateSerialOut`, protocol.md), so this phase creates that wire tree, sending side only. These are the first types in `shared/src/protocol/`, the only module allowed to name bitcode.

- [ ] **Step 1: Write the failing test**

Create `shared/src/protocol/geometry_serial.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_copies_each_geometry_type_field_for_field() {
        assert_eq!(
            WorldPointSerialOut::from(&WorldPoint { x: -3, y: 9 }),
            WorldPointSerialOut { x: -3, y: 9 },
        );
        assert_eq!(
            SubpixelPointSerial::from(&SubpixelPoint { x: 1024, y: -1 }),
            SubpixelPointSerial { x: 1024, y: -1 },
        );
        assert_eq!(
            SubpixelVectorSerialOut::from(&SubpixelVector { x: -7603, y: 7603 }),
            SubpixelVectorSerialOut { x: -7603, y: 7603 },
        );
        assert_eq!(
            LatticeCoordinateSerialOut::from(&LatticeCoordinate { i: 4, j: -2 }),
            LatticeCoordinateSerialOut { i: 4, j: -2 },
        );
    }
}
```

Create `shared/src/protocol/mod.rs`:

```rust
pub mod geometry_serial;

pub use geometry_serial::*;
```

In `shared/src/lib.rs`, replace:

```rust
pub mod organism;
```

with:

```rust
pub mod organism;
pub mod protocol;
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib protocol::geometry_serial`
Expected: compilation fails; the first error is `` error[E0422]: cannot find struct, variant or union type `WorldPoint` in this scope ``.

- [ ] **Step 3: Implement**

Replace the whole of `shared/src/protocol/geometry_serial.rs` with:

```rust
use bitcode::{Decode, Encode};

use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, WorldPoint};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct WorldPointSerialOut {
    pub x: i32,
    pub y: i32,
}

impl From<&WorldPoint> for WorldPointSerialOut {
    fn from(world_point: &WorldPoint) -> WorldPointSerialOut {
        WorldPointSerialOut {
            x: world_point.x,
            y: world_point.y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SubpixelPointSerial {
    pub x: i32,
    pub y: i32,
}

impl From<&SubpixelPoint> for SubpixelPointSerial {
    fn from(subpixel_point: &SubpixelPoint) -> SubpixelPointSerial {
        SubpixelPointSerial {
            x: subpixel_point.x,
            y: subpixel_point.y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SubpixelVectorSerialOut {
    pub x: i32,
    pub y: i32,
}

impl From<&SubpixelVector> for SubpixelVectorSerialOut {
    fn from(subpixel_vector: &SubpixelVector) -> SubpixelVectorSerialOut {
        SubpixelVectorSerialOut {
            x: subpixel_vector.x,
            y: subpixel_vector.y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct LatticeCoordinateSerialOut {
    pub i: i32,
    pub j: i32,
}

impl From<&LatticeCoordinate> for LatticeCoordinateSerialOut {
    fn from(lattice_coordinate: &LatticeCoordinate) -> LatticeCoordinateSerialOut {
        LatticeCoordinateSerialOut {
            i: lattice_coordinate.i,
            j: lattice_coordinate.j,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_copies_each_geometry_type_field_for_field() {
        assert_eq!(
            WorldPointSerialOut::from(&WorldPoint { x: -3, y: 9 }),
            WorldPointSerialOut { x: -3, y: 9 },
        );
        assert_eq!(
            SubpixelPointSerial::from(&SubpixelPoint { x: 1024, y: -1 }),
            SubpixelPointSerial { x: 1024, y: -1 },
        );
        assert_eq!(
            SubpixelVectorSerialOut::from(&SubpixelVector { x: -7603, y: 7603 }),
            SubpixelVectorSerialOut { x: -7603, y: 7603 },
        );
        assert_eq!(
            LatticeCoordinateSerialOut::from(&LatticeCoordinate { i: 4, j: -2 }),
            LatticeCoordinateSerialOut { i: 4, j: -2 },
        );
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib protocol::geometry_serial`
Expected: no warnings; `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 214 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 215 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/geometry_serial.rs shared/src/protocol/mod.rs shared/src/lib.rs
git status --short
git commit -m "geometry wire types"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 22: Organism wire types

**Files:**
- Create: `shared/src/protocol/organism_serial.rs`
- Modify: `shared/src/protocol/mod.rs`
- Test: inline `mod tests` in `shared/src/protocol/organism_serial.rs`

Field for field the organism, its cell bitmap (through the `CellOccupancy` accessors) and every ability phase, ticks as `u32`.

- [ ] **Step 1: Write the failing test**

Create `shared/src/protocol/organism_serial.rs`:

```rust
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
```

Replace the whole of `shared/src/protocol/mod.rs` with:

```rust
pub mod geometry_serial;
pub mod organism_serial;

pub use geometry_serial::*;
pub use organism_serial::*;
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib protocol::organism_serial`
Expected: compilation fails; the first error is `` error[E0425]: cannot find type `CellOccupancy` in this scope ``.

- [ ] **Step 3: Implement**

Replace the whole of `shared/src/protocol/organism_serial.rs` with:

```rust
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
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib protocol::organism_serial`
Expected: no warnings; `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 215 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 217 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/organism_serial.rs shared/src/protocol/mod.rs
git status --short
git commit -m "organism wire types"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 23: Member wire types

**Files:**
- Create: `shared/src/protocol/member_serial.rs`
- Modify: `shared/src/protocol/mod.rs`
- Test: inline `mod tests` in `shared/src/protocol/member_serial.rs`

The member with its role, loadout, team, score and organism. `LoadoutSerial`, `AppearanceSerial` and the kind mirrors are `...Serial` because client requests carry the same shapes (protocol.md).

- [ ] **Step 1: Write the failing test**

Create `shared/src/protocol/member_serial.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::geometry::WorldPoint;
    use crate::member::MemberId;

    #[test]
    fn from_copies_a_member_with_its_loadout_and_team() {
        let mut member: Member = test_fixture::create_participant_with_organism(MemberId(7), WorldPoint { x: 1, y: 2 });
        member.team = Some(TeamKind::Pink);
        member.score = Score {
            kills: 3,
            deaths: 1,
            wins: 2,
        };

        let member_serial_out: MemberSerialOut = MemberSerialOut::from(&member);

        assert_eq!(member_serial_out.member_id, 7);
        assert_eq!(member_serial_out.screen_name, "player 7");
        assert_eq!(member_serial_out.role, MemberRoleKindSerialOut::Participant);
        assert_eq!(
            member_serial_out.loadout,
            Some(LoadoutSerial {
                appearance: AppearanceSerial {
                    color: OrganismColorKindSerial::Ocean,
                    skin: SkinKindSerial::Circles,
                },
                first: FirstAbilityKindSerial::Extend,
                second: SecondAbilityKindSerial::Immortality,
                third: ThirdAbilityKindSerial::Neutralize,
            }),
        );
        assert_eq!(member_serial_out.team, Some(TeamKindSerial::Pink));
        assert_eq!(
            member_serial_out.score,
            ScoreSerialOut {
                kills: 3,
                deaths: 1,
                wins: 2,
            },
        );
        assert!(member_serial_out.organism.is_some());
    }
}
```

Replace the whole of `shared/src/protocol/mod.rs` with:

```rust
pub mod geometry_serial;
pub mod member_serial;
pub mod organism_serial;

pub use geometry_serial::*;
pub use member_serial::*;
pub use organism_serial::*;
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib protocol::member_serial`
Expected: compilation fails; the first error is `` error[E0425]: cannot find type `Member` in this scope ``.

- [ ] **Step 3: Implement**

Replace the whole of `shared/src/protocol/member_serial.rs` with:

```rust
use bitcode::{Decode, Encode};

use crate::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use crate::member::{Appearance, Member, MemberRoleKind, OrganismColorKind, Score, SkinKind, TeamKind};
use crate::protocol::OrganismSerialOut;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct MemberSerialOut {
    pub member_id: u32,
    pub screen_name: String,
    pub role: MemberRoleKindSerialOut,
    pub loadout: Option<LoadoutSerial>,
    pub team: Option<TeamKindSerial>,
    pub score: ScoreSerialOut,
    pub organism: Option<OrganismSerialOut>,
}

impl From<&Member> for MemberSerialOut {
    fn from(member: &Member) -> MemberSerialOut {
        MemberSerialOut {
            member_id: member.member_id.0,
            screen_name: member.screen_name.clone(),
            role: MemberRoleKindSerialOut::from(&member.role),
            loadout: member.loadout.as_ref().map(LoadoutSerial::from),
            team: member.team.as_ref().map(TeamKindSerial::from),
            score: ScoreSerialOut::from(&member.score),
            organism: member.organism.as_ref().map(OrganismSerialOut::from),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum MemberRoleKindSerialOut {
    Participant,
    Spectator,
}

impl From<&MemberRoleKind> for MemberRoleKindSerialOut {
    fn from(role: &MemberRoleKind) -> MemberRoleKindSerialOut {
        match role {
            MemberRoleKind::Participant => MemberRoleKindSerialOut::Participant,
            MemberRoleKind::Spectator => MemberRoleKindSerialOut::Spectator,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ScoreSerialOut {
    pub kills: u32,
    pub deaths: u32,
    pub wins: u32,
}

impl From<&Score> for ScoreSerialOut {
    fn from(score: &Score) -> ScoreSerialOut {
        ScoreSerialOut {
            kills: score.kills,
            deaths: score.deaths,
            wins: score.wins,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct LoadoutSerial {
    pub appearance: AppearanceSerial,
    pub first: FirstAbilityKindSerial,
    pub second: SecondAbilityKindSerial,
    pub third: ThirdAbilityKindSerial,
}

impl From<&Loadout> for LoadoutSerial {
    fn from(loadout: &Loadout) -> LoadoutSerial {
        LoadoutSerial {
            appearance: AppearanceSerial::from(&loadout.appearance),
            first: FirstAbilityKindSerial::from(&loadout.first),
            second: SecondAbilityKindSerial::from(&loadout.second),
            third: ThirdAbilityKindSerial::from(&loadout.third),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum FirstAbilityKindSerial {
    Extend,
    Compress,
}

impl From<&FirstAbilityKind> for FirstAbilityKindSerial {
    fn from(first: &FirstAbilityKind) -> FirstAbilityKindSerial {
        match first {
            FirstAbilityKind::Extend => FirstAbilityKindSerial::Extend,
            FirstAbilityKind::Compress => FirstAbilityKindSerial::Compress,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SecondAbilityKindSerial {
    Immortality,
    Freeze,
}

impl From<&SecondAbilityKind> for SecondAbilityKindSerial {
    fn from(second: &SecondAbilityKind) -> SecondAbilityKindSerial {
        match second {
            SecondAbilityKind::Immortality => SecondAbilityKindSerial::Immortality,
            SecondAbilityKind::Freeze => SecondAbilityKindSerial::Freeze,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum ThirdAbilityKindSerial {
    Neutralize,
    Toxin,
}

impl From<&ThirdAbilityKind> for ThirdAbilityKindSerial {
    fn from(third: &ThirdAbilityKind) -> ThirdAbilityKindSerial {
        match third {
            ThirdAbilityKind::Neutralize => ThirdAbilityKindSerial::Neutralize,
            ThirdAbilityKind::Toxin => ThirdAbilityKindSerial::Toxin,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct AppearanceSerial {
    pub color: OrganismColorKindSerial,
    pub skin: SkinKindSerial,
}

impl From<&Appearance> for AppearanceSerial {
    fn from(appearance: &Appearance) -> AppearanceSerial {
        AppearanceSerial {
            color: OrganismColorKindSerial::from(&appearance.color),
            skin: SkinKindSerial::from(&appearance.skin),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum OrganismColorKindSerial {
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

impl From<&OrganismColorKind> for OrganismColorKindSerial {
    fn from(color: &OrganismColorKind) -> OrganismColorKindSerial {
        match color {
            OrganismColorKind::Fire => OrganismColorKindSerial::Fire,
            OrganismColorKind::Camel => OrganismColorKindSerial::Camel,
            OrganismColorKind::Clay => OrganismColorKindSerial::Clay,
            OrganismColorKind::Sun => OrganismColorKindSerial::Sun,
            OrganismColorKind::Leaf => OrganismColorKindSerial::Leaf,
            OrganismColorKind::Lime => OrganismColorKindSerial::Lime,
            OrganismColorKind::Sky => OrganismColorKindSerial::Sky,
            OrganismColorKind::Lake => OrganismColorKindSerial::Lake,
            OrganismColorKind::Ocean => OrganismColorKindSerial::Ocean,
            OrganismColorKind::Royal => OrganismColorKindSerial::Royal,
            OrganismColorKind::Petal => OrganismColorKindSerial::Petal,
            OrganismColorKind::Hot => OrganismColorKindSerial::Hot,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SkinKindSerial {
    Grid,
    Circles,
    Ghost,
    None,
}

impl From<&SkinKind> for SkinKindSerial {
    fn from(skin: &SkinKind) -> SkinKindSerial {
        match skin {
            SkinKind::Grid => SkinKindSerial::Grid,
            SkinKind::Circles => SkinKindSerial::Circles,
            SkinKind::Ghost => SkinKindSerial::Ghost,
            SkinKind::None => SkinKindSerial::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum TeamKindSerial {
    Red,
    Blue,
    Green,
    Pink,
}

impl From<&TeamKind> for TeamKindSerial {
    fn from(team: &TeamKind) -> TeamKindSerial {
        match team {
            TeamKind::Red => TeamKindSerial::Red,
            TeamKind::Blue => TeamKindSerial::Blue,
            TeamKind::Green => TeamKindSerial::Green,
            TeamKind::Pink => TeamKindSerial::Pink,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::geometry::WorldPoint;
    use crate::member::MemberId;

    #[test]
    fn from_copies_a_member_with_its_loadout_and_team() {
        let mut member: Member = test_fixture::create_participant_with_organism(MemberId(7), WorldPoint { x: 1, y: 2 });
        member.team = Some(TeamKind::Pink);
        member.score = Score {
            kills: 3,
            deaths: 1,
            wins: 2,
        };

        let member_serial_out: MemberSerialOut = MemberSerialOut::from(&member);

        assert_eq!(member_serial_out.member_id, 7);
        assert_eq!(member_serial_out.screen_name, "player 7");
        assert_eq!(member_serial_out.role, MemberRoleKindSerialOut::Participant);
        assert_eq!(
            member_serial_out.loadout,
            Some(LoadoutSerial {
                appearance: AppearanceSerial {
                    color: OrganismColorKindSerial::Ocean,
                    skin: SkinKindSerial::Circles,
                },
                first: FirstAbilityKindSerial::Extend,
                second: SecondAbilityKindSerial::Immortality,
                third: ThirdAbilityKindSerial::Neutralize,
            }),
        );
        assert_eq!(member_serial_out.team, Some(TeamKindSerial::Pink));
        assert_eq!(
            member_serial_out.score,
            ScoreSerialOut {
                kills: 3,
                deaths: 1,
                wins: 2,
            },
        );
        assert!(member_serial_out.organism.is_some());
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib protocol::member_serial`
Expected: no warnings; `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 217 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 218 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/member_serial.rs shared/src/protocol/mod.rs
git status --short
git commit -m "member wire types"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 24: Game state wire types

**Files:**
- Create: `shared/src/protocol/game_state_serial.rs`
- Modify: `shared/src/protocol/mod.rs`
- Test: inline `mod tests` in `shared/src/protocol/game_state_serial.rs`

The root of the snapshot: settings, generator state, world, round and the members as a list in ascending id in place of the `BTreeMap`.

- [ ] **Step 1: Write the failing test**

Create `shared/src/protocol/game_state_serial.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Tick, test_fixture};
    use crate::member::MemberId;

    #[test]
    fn from_lists_members_in_ascending_id() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Ellipse, 800);
        state.tick = Tick(12);
        state.next_member_id = MemberId(9);

        for member_id in [MemberId(8), MemberId(2), MemberId(5)] {
            state.members.insert(member_id, test_fixture::create_participant(member_id));
        }

        let state_serial_out: GameStateSerialOut = GameStateSerialOut::from(&state);
        let member_ids: Vec<u32> = state_serial_out.members.iter().map(|member| member.member_id).collect();

        assert_eq!(member_ids, vec![2, 5, 8]);
        assert_eq!(state_serial_out.tick, 12);
        assert_eq!(state_serial_out.next_member_id, 9);
        assert_eq!(
            state_serial_out.rng,
            Pcg32SerialOut {
                state: state.rng.state(),
                increment: state.rng.increment(),
            },
        );
        assert_eq!(
            state_serial_out.round,
            Some(RoundStateSerialOut {
                phase: RoundPhaseSerialOut::Waiting,
                phase_started_at: 0,
            }),
        );
    }

    #[test]
    fn from_copies_the_settings_and_the_world() {
        let state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Ellipse, 800);
        let state_serial_out: GameStateSerialOut = GameStateSerialOut::from(&state);
        let bounds_serial_out: WorldBoundsSerialOut = WorldBoundsSerialOut {
            left: 0,
            top: 0,
            width: 819_200,
            height: 819_200,
        };

        assert_eq!(
            state_serial_out.settings,
            GameSettingsSerialOut {
                title: String::from("Fixture game"),
                mode: GameModeKindSerial::Skirmish,
                world_shape: WorldShapeKindSerial::Ellipse,
                world_width_pixels: 800,
                world_height_pixels: 800,
                player_minimum: None,
                player_cap: 8,
                team_count: Some(2),
                leaderboard_length: 10,
            },
        );
        assert_eq!(
            state_serial_out.world,
            WorldSerialOut {
                shape: WorldShapeKindSerial::Ellipse,
                bounds: bounds_serial_out,
                initial_bounds: bounds_serial_out,
            },
        );
    }
}
```

Replace the whole of `shared/src/protocol/mod.rs` with:

```rust
pub mod game_state_serial;
pub mod geometry_serial;
pub mod member_serial;
pub mod organism_serial;

pub use game_state_serial::*;
pub use geometry_serial::*;
pub use member_serial::*;
pub use organism_serial::*;
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib protocol::game_state_serial`
Expected: compilation fails; the first error is `` error[E0425]: cannot find type `GameState` in this scope ``.

- [ ] **Step 3: Implement**

Replace the whole of `shared/src/protocol/game_state_serial.rs` with:

```rust
use bitcode::{Decode, Encode};

use crate::game::{GameModeKind, GameSettings, GameState};
use crate::protocol::MemberSerialOut;
use crate::random::Pcg32;
use crate::round::{RoundPhase, RoundState};
use crate::world::{World, WorldBounds, WorldShapeKind};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct GameStateSerialOut {
    pub tick: u32,
    pub settings: GameSettingsSerialOut,
    pub rng: Pcg32SerialOut,
    pub world: WorldSerialOut,
    pub round: Option<RoundStateSerialOut>,
    /// Ascending `member_id`.
    pub members: Vec<MemberSerialOut>,
    pub next_member_id: u32,
}

impl From<&GameState> for GameStateSerialOut {
    fn from(state: &GameState) -> GameStateSerialOut {
        GameStateSerialOut {
            tick: state.tick.0,
            settings: GameSettingsSerialOut::from(&state.settings),
            rng: Pcg32SerialOut::from(&state.rng),
            world: WorldSerialOut::from(&state.world),
            round: state.round.as_ref().map(RoundStateSerialOut::from),
            members: state.members.values().map(MemberSerialOut::from).collect(),
            next_member_id: state.next_member_id.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct GameSettingsSerialOut {
    pub title: String,
    pub mode: GameModeKindSerial,
    pub world_shape: WorldShapeKindSerial,
    pub world_width_pixels: u32,
    pub world_height_pixels: u32,
    pub player_minimum: Option<u8>,
    pub player_cap: u8,
    pub team_count: Option<u8>,
    pub leaderboard_length: u8,
}

impl From<&GameSettings> for GameSettingsSerialOut {
    fn from(settings: &GameSettings) -> GameSettingsSerialOut {
        GameSettingsSerialOut {
            title: settings.title.clone(),
            mode: GameModeKindSerial::from(&settings.mode),
            world_shape: WorldShapeKindSerial::from(&settings.world_shape),
            world_width_pixels: settings.world_width_pixels,
            world_height_pixels: settings.world_height_pixels,
            player_minimum: settings.player_minimum,
            player_cap: settings.player_cap,
            team_count: settings.team_count,
            leaderboard_length: settings.leaderboard_length,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum GameModeKindSerial {
    FreeForAll,
    Skirmish,
    Survival,
}

impl From<&GameModeKind> for GameModeKindSerial {
    fn from(mode: &GameModeKind) -> GameModeKindSerial {
        match mode {
            GameModeKind::FreeForAll => GameModeKindSerial::FreeForAll,
            GameModeKind::Skirmish => GameModeKindSerial::Skirmish,
            GameModeKind::Survival => GameModeKindSerial::Survival,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum WorldShapeKindSerial {
    Rectangle,
    Ellipse,
}

impl From<&WorldShapeKind> for WorldShapeKindSerial {
    fn from(world_shape: &WorldShapeKind) -> WorldShapeKindSerial {
        match world_shape {
            WorldShapeKind::Rectangle => WorldShapeKindSerial::Rectangle,
            WorldShapeKind::Ellipse => WorldShapeKindSerial::Ellipse,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct Pcg32SerialOut {
    pub state: u64,
    pub increment: u64,
}

impl From<&Pcg32> for Pcg32SerialOut {
    fn from(rng: &Pcg32) -> Pcg32SerialOut {
        Pcg32SerialOut {
            state: rng.state(),
            increment: rng.increment(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct WorldSerialOut {
    pub shape: WorldShapeKindSerial,
    pub bounds: WorldBoundsSerialOut,
    pub initial_bounds: WorldBoundsSerialOut,
}

impl From<&World> for WorldSerialOut {
    fn from(world: &World) -> WorldSerialOut {
        WorldSerialOut {
            shape: WorldShapeKindSerial::from(&world.shape),
            bounds: WorldBoundsSerialOut::from(&world.bounds),
            initial_bounds: WorldBoundsSerialOut::from(&world.initial_bounds),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct WorldBoundsSerialOut {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

impl From<&WorldBounds> for WorldBoundsSerialOut {
    fn from(bounds: &WorldBounds) -> WorldBoundsSerialOut {
        WorldBoundsSerialOut {
            left: bounds.left.0,
            top: bounds.top.0,
            width: bounds.width.0,
            height: bounds.height.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct RoundStateSerialOut {
    pub phase: RoundPhaseSerialOut,
    pub phase_started_at: u32,
}

impl From<&RoundState> for RoundStateSerialOut {
    fn from(round: &RoundState) -> RoundStateSerialOut {
        RoundStateSerialOut {
            phase: RoundPhaseSerialOut::from(&round.phase),
            phase_started_at: round.phase_started_at.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum RoundPhaseSerialOut {
    Waiting,
    PreRound,
    Playing,
    PostRound,
}

impl From<&RoundPhase> for RoundPhaseSerialOut {
    fn from(phase: &RoundPhase) -> RoundPhaseSerialOut {
        match phase {
            RoundPhase::Waiting => RoundPhaseSerialOut::Waiting,
            RoundPhase::PreRound => RoundPhaseSerialOut::PreRound,
            RoundPhase::Playing => RoundPhaseSerialOut::Playing,
            RoundPhase::PostRound => RoundPhaseSerialOut::PostRound,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Tick, test_fixture};
    use crate::member::MemberId;

    #[test]
    fn from_lists_members_in_ascending_id() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Ellipse, 800);
        state.tick = Tick(12);
        state.next_member_id = MemberId(9);

        for member_id in [MemberId(8), MemberId(2), MemberId(5)] {
            state.members.insert(member_id, test_fixture::create_participant(member_id));
        }

        let state_serial_out: GameStateSerialOut = GameStateSerialOut::from(&state);
        let member_ids: Vec<u32> = state_serial_out.members.iter().map(|member| member.member_id).collect();

        assert_eq!(member_ids, vec![2, 5, 8]);
        assert_eq!(state_serial_out.tick, 12);
        assert_eq!(state_serial_out.next_member_id, 9);
        assert_eq!(
            state_serial_out.rng,
            Pcg32SerialOut {
                state: state.rng.state(),
                increment: state.rng.increment(),
            },
        );
        assert_eq!(
            state_serial_out.round,
            Some(RoundStateSerialOut {
                phase: RoundPhaseSerialOut::Waiting,
                phase_started_at: 0,
            }),
        );
    }

    #[test]
    fn from_copies_the_settings_and_the_world() {
        let state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Ellipse, 800);
        let state_serial_out: GameStateSerialOut = GameStateSerialOut::from(&state);
        let bounds_serial_out: WorldBoundsSerialOut = WorldBoundsSerialOut {
            left: 0,
            top: 0,
            width: 819_200,
            height: 819_200,
        };

        assert_eq!(
            state_serial_out.settings,
            GameSettingsSerialOut {
                title: String::from("Fixture game"),
                mode: GameModeKindSerial::Skirmish,
                world_shape: WorldShapeKindSerial::Ellipse,
                world_width_pixels: 800,
                world_height_pixels: 800,
                player_minimum: None,
                player_cap: 8,
                team_count: Some(2),
                leaderboard_length: 10,
            },
        );
        assert_eq!(
            state_serial_out.world,
            WorldSerialOut {
                shape: WorldShapeKindSerial::Ellipse,
                bounds: bounds_serial_out,
                initial_bounds: bounds_serial_out,
            },
        );
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib protocol::game_state_serial`
Expected: no warnings; `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 218 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 220 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/game_state_serial.rs shared/src/protocol/mod.rs
git status --short
git commit -m "game state wire types"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 25: State checksum

**Files:**
- Create: `shared/src/protocol/protocol.rs`
- Create: `shared/src/protocol/state_checksum.rs`
- Modify: `shared/src/protocol/mod.rs`
- Modify: `shared/src/organism/growth_chance_table.rs`
- Test: inline `mod tests` in `shared/src/protocol/state_checksum.rs`

`get_state_checksum` is FNV-1a 64 over `protocol::encode_game_state(&GameStateSerialOut::from(&state))`. Canonical state (re-tightened bitmaps, members in id order) makes equal states hash equally. The golden chance-table digest test switches to the shared FNV-1a function and keeps its golden value.

- [ ] **Step 1: Write the failing test**

Create `shared/src/protocol/state_checksum.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, WorldPoint};
    use crate::member::MemberId;
    use crate::organism::CellOccupancy;
    use crate::world::WorldShapeKind;

    fn create_state() -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.members.insert(
            MemberId(0),
            test_fixture::create_participant_with_organism(MemberId(0), WorldPoint { x: 100, y: 100 }),
        );

        state
    }

    #[test]
    fn get_fnv1a_64_digest_matches_the_reference_vectors() {
        assert_eq!(get_fnv1a_64_digest(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(get_fnv1a_64_digest(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(get_fnv1a_64_digest(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn get_state_checksum_is_equal_for_equal_states() {
        assert_eq!(get_state_checksum(&create_state()), get_state_checksum(&create_state()));
    }

    #[test]
    fn get_state_checksum_covers_the_random_number_generator() {
        let state: GameState = create_state();
        let mut drawn_state: GameState = create_state();
        drawn_state.rng.next_u32();

        assert_ne!(get_state_checksum(&state), get_state_checksum(&drawn_state));
    }

    #[test]
    fn get_state_checksum_covers_the_cells() {
        let state: GameState = create_state();
        let mut grown_state: GameState = create_state();
        test_fixture::get_organism_mut(&mut grown_state, MemberId(0))
            .cells
            .insert(LatticeCoordinate { i: 1, j: 0 });

        assert_ne!(get_state_checksum(&state), get_state_checksum(&grown_state));
    }

    #[test]
    fn get_state_checksum_ignores_the_insertion_order_of_retightened_cells() {
        let mut first_state: GameState = create_state();
        let mut second_state: GameState = create_state();

        for (i, j) in [(1, 0), (0, 1), (-4, -4)] {
            test_fixture::get_organism_mut(&mut first_state, MemberId(0))
                .cells
                .insert(LatticeCoordinate { i, j });
        }

        for (i, j) in [(0, 1), (1, 0)] {
            test_fixture::get_organism_mut(&mut second_state, MemberId(0))
                .cells
                .insert(LatticeCoordinate { i, j });
        }

        let first_cells: &mut CellOccupancy = &mut test_fixture::get_organism_mut(&mut first_state, MemberId(0)).cells;
        first_cells.remove(LatticeCoordinate { i: -4, j: -4 });
        first_cells.retighten();
        test_fixture::get_organism_mut(&mut second_state, MemberId(0)).cells.retighten();

        assert_eq!(get_state_checksum(&first_state), get_state_checksum(&second_state));
    }
}
```

Replace the whole of `shared/src/protocol/mod.rs` with:

```rust
pub mod game_state_serial;
pub mod geometry_serial;
pub mod member_serial;
pub mod organism_serial;
pub mod state_checksum;

pub use game_state_serial::*;
pub use geometry_serial::*;
pub use member_serial::*;
pub use organism_serial::*;
pub use state_checksum::*;
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --lib protocol::state_checksum`
Expected: compilation fails; the first error is `` error[E0425]: cannot find type `GameState` in this scope ``.

- [ ] **Step 3: Implement**

Create `shared/src/protocol/protocol.rs`:

```rust
use crate::protocol::GameStateSerialOut;

pub fn encode_game_state(state_serial_out: &GameStateSerialOut) -> Vec<u8> {
    bitcode::encode(state_serial_out)
}
```

Replace the whole of `shared/src/protocol/mod.rs` with:

```rust
pub mod game_state_serial;
pub mod geometry_serial;
pub mod member_serial;
pub mod organism_serial;
pub mod protocol;
pub mod state_checksum;

pub use game_state_serial::*;
pub use geometry_serial::*;
pub use member_serial::*;
pub use organism_serial::*;
pub use protocol::*;
pub use state_checksum::*;
```

Replace the whole of `shared/src/protocol/state_checksum.rs` with:

```rust
use crate::game::GameState;
use crate::protocol;
use crate::protocol::GameStateSerialOut;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64 over the wire encoding of the state.
pub fn get_state_checksum(state: &GameState) -> u64 {
    let state_bytes: Vec<u8> = protocol::encode_game_state(&GameStateSerialOut::from(state));

    get_fnv1a_64_digest(&state_bytes)
}

pub fn get_fnv1a_64_digest(bytes: &[u8]) -> u64 {
    let mut digest: u64 = FNV_OFFSET_BASIS;

    for byte in bytes {
        digest ^= u64::from(*byte);
        digest = digest.wrapping_mul(FNV_PRIME);
    }

    digest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry::{LatticeCoordinate, WorldPoint};
    use crate::member::MemberId;
    use crate::organism::CellOccupancy;
    use crate::world::WorldShapeKind;

    fn create_state() -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.members.insert(
            MemberId(0),
            test_fixture::create_participant_with_organism(MemberId(0), WorldPoint { x: 100, y: 100 }),
        );

        state
    }

    #[test]
    fn get_fnv1a_64_digest_matches_the_reference_vectors() {
        assert_eq!(get_fnv1a_64_digest(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(get_fnv1a_64_digest(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(get_fnv1a_64_digest(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn get_state_checksum_is_equal_for_equal_states() {
        assert_eq!(get_state_checksum(&create_state()), get_state_checksum(&create_state()));
    }

    #[test]
    fn get_state_checksum_covers_the_random_number_generator() {
        let state: GameState = create_state();
        let mut drawn_state: GameState = create_state();
        drawn_state.rng.next_u32();

        assert_ne!(get_state_checksum(&state), get_state_checksum(&drawn_state));
    }

    #[test]
    fn get_state_checksum_covers_the_cells() {
        let state: GameState = create_state();
        let mut grown_state: GameState = create_state();
        test_fixture::get_organism_mut(&mut grown_state, MemberId(0))
            .cells
            .insert(LatticeCoordinate { i: 1, j: 0 });

        assert_ne!(get_state_checksum(&state), get_state_checksum(&grown_state));
    }

    #[test]
    fn get_state_checksum_ignores_the_insertion_order_of_retightened_cells() {
        let mut first_state: GameState = create_state();
        let mut second_state: GameState = create_state();

        for (i, j) in [(1, 0), (0, 1), (-4, -4)] {
            test_fixture::get_organism_mut(&mut first_state, MemberId(0))
                .cells
                .insert(LatticeCoordinate { i, j });
        }

        for (i, j) in [(0, 1), (1, 0)] {
            test_fixture::get_organism_mut(&mut second_state, MemberId(0))
                .cells
                .insert(LatticeCoordinate { i, j });
        }

        let first_cells: &mut CellOccupancy = &mut test_fixture::get_organism_mut(&mut first_state, MemberId(0)).cells;
        first_cells.remove(LatticeCoordinate { i: -4, j: -4 });
        first_cells.retighten();
        test_fixture::get_organism_mut(&mut second_state, MemberId(0)).cells.retighten();

        assert_eq!(get_state_checksum(&first_state), get_state_checksum(&second_state));
    }
}
```

In `shared/src/organism/growth_chance_table.rs`, replace:

```rust
mod tests {
    use super::*;
```

with:

```rust
mod tests {
    use super::*;
    use crate::protocol::state_checksum;
```

In `shared/src/organism/growth_chance_table.rs`, replace:

```rust
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

```

with:

```rust
    const GOLDEN_TABLE_DIGEST: u64 = 0x75a8_9b54_1a01_86e5;

    fn get_tables_digest(tables: &GrowthChanceTables) -> u64 {
        let mut threshold_bytes: Vec<u8> = Vec::new();

        for growth_state in GROWTH_STATES {
            let table: &GrowthChanceTable = tables.get(growth_state);
            let thresholds: Vec<u64> = [table.birth_thresholds(), table.death_thresholds()].concat();

            for threshold in thresholds {
                threshold_bytes.extend(threshold.to_le_bytes());
            }
        }

        state_checksum::get_fnv1a_64_digest(&threshold_bytes)
    }

```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p shared --lib -- protocol::state_checksum organism::growth_chance_table`
Expected: no warnings; `test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 212 filtered out`.

Run: `cargo test -p shared --lib`
Expected: no warnings; `test result: ok. 225 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/protocol.rs shared/src/protocol/state_checksum.rs shared/src/protocol/mod.rs shared/src/organism/growth_chance_table.rs
git status --short
git commit -m "state checksum"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 26: Scripted game determinism tests

**Files:**
- Create: `shared/tests/determinism.rs`
- Create: `scripts/test/regenerate-replay-fixtures.sh`
- Create: `shared/tests/fixtures/scripted_game.checksums`
- Test: `shared/tests/determinism.rs` (integration test)

Sixteen scripted players in a survival game, every loadout twice, wander towards random targets and press every key at random (zero aims included); a spectator joins, a player leaves at tick 4000, a late joiner arrives at tick 4001 (its spawn is rejected while a round is in progress) and an appearance changes at tick 6000. The script draws from its own `Pcg32`, so it is a pure function of the states it reads. 10,000 ticks run twice must give identical checksum sequences, and the sequence must equal the committed golden file. Each run takes about 6 s in a debug build.

- [ ] **Step 1: Write the failing test**

Create `shared/tests/determinism.rs`:

```rust
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use shared::ability::{AbilityPressSet, AimVector, FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use shared::game;
use shared::game::{GameModeKind, GameSettings, GameState, InputBundle, MemberEvent, PlayerTickInput, Tick};
use shared::geometry::WorldPoint;
use shared::member::{Appearance, MemberId, MemberRoleKind, OrganismColorKind, SkinKind};
use shared::protocol::state_checksum;
use shared::random::Pcg32;
use shared::world::WorldShapeKind;

const SCRIPTED_TICK_COUNT: u32 = 10_000;
const SCRIPTED_PLAYER_COUNT: u32 = 16;
const GAME_SEED: u64 = 0x5c41_97ed_0000_0001;
const SCRIPT_SEED: u64 = 0x5c41_97ed_0000_0002;
const WORLD_SIZE_PIXELS: u32 = 800;
const CURSOR_TARGET_MARGIN_PIXELS: u32 = 100;
const CURSOR_STEP_PIXELS: i32 = 3;
const PRESS_CHANCE_DENOMINATOR: u32 = 32;
const AIM_EXTENT_PIXELS: u32 = 100;
const LEAVE_TICK: Tick = Tick(4000);
const LEAVING_MEMBER_ID: MemberId = MemberId(3);
const REJOIN_TICK: Tick = Tick(4001);
const APPEARANCE_CHANGE_TICK: Tick = Tick(6000);
const APPEARANCE_CHANGING_MEMBER_ID: MemberId = MemberId(5);
const COLORS: [OrganismColorKind; 4] = [
    OrganismColorKind::Fire,
    OrganismColorKind::Leaf,
    OrganismColorKind::Lake,
    OrganismColorKind::Royal,
];
const SKINS: [SkinKind; 4] = [SkinKind::Grid, SkinKind::Circles, SkinKind::Ghost, SkinKind::None];
const PRESSES: [AbilityPressSet; 4] = [
    AbilityPressSet::FIRST,
    AbilityPressSet::SECOND,
    AbilityPressSet::THIRD,
    AbilityPressSet::FOURTH,
];

/// Players who wander towards random targets and press at random.
struct ScriptedPlayers {
    script_rng: Pcg32,
    cursor_targets: BTreeMap<MemberId, WorldPoint>,
}

impl ScriptedPlayers {
    fn new() -> ScriptedPlayers {
        ScriptedPlayers {
            script_rng: Pcg32::from_seed(SCRIPT_SEED),
            cursor_targets: BTreeMap::new(),
        }
    }

    fn create_bundle(&mut self, state: &GameState) -> InputBundle {
        let tick: Tick = state.tick.next();
        let member_events: Vec<MemberEvent> = get_member_events(state, tick);
        let mut player_inputs: Vec<PlayerTickInput> = Vec::new();

        for member in state.members.values() {
            let Some(organism) = &member.organism else {
                continue;
            };

            player_inputs.push(self.create_player_input(member.member_id, organism.cursor));
        }

        InputBundle {
            tick,
            member_events,
            player_inputs,
        }
    }

    fn create_player_input(&mut self, member_id: MemberId, cursor: WorldPoint) -> PlayerTickInput {
        let cursor_target: WorldPoint = self.get_cursor_target(member_id, cursor);
        let next_cursor: WorldPoint = WorldPoint {
            x: cursor.x + (cursor_target.x - cursor.x).clamp(-CURSOR_STEP_PIXELS, CURSOR_STEP_PIXELS),
            y: cursor.y + (cursor_target.y - cursor.y).clamp(-CURSOR_STEP_PIXELS, CURSOR_STEP_PIXELS),
        };
        let mut ability_presses: AbilityPressSet = AbilityPressSet::NONE;

        for press in PRESSES {
            let is_pressed: bool = self.script_rng.below(PRESS_CHANCE_DENOMINATOR) == 0;

            if is_pressed {
                ability_presses = ability_presses.with(press);
            }
        }

        let is_shot_pressed: bool =
            ability_presses.contains(AbilityPressSet::FIRST) || ability_presses.contains(AbilityPressSet::SECOND);
        let aim: Option<AimVector> = if is_shot_pressed { Some(self.draw_aim()) } else { None };

        PlayerTickInput {
            member_id,
            cursor: next_cursor,
            ability_presses,
            aim,
        }
    }

    /// A new target once the cursor reaches the current one.
    fn get_cursor_target(&mut self, member_id: MemberId, cursor: WorldPoint) -> WorldPoint {
        let current_target: Option<WorldPoint> = self.cursor_targets.get(&member_id).copied();

        if let Some(cursor_target) = current_target.filter(|cursor_target| *cursor_target != cursor) {
            return cursor_target;
        }

        let cursor_target: WorldPoint = WorldPoint {
            x: self.draw_target_coordinate(),
            y: self.draw_target_coordinate(),
        };
        self.cursor_targets.insert(member_id, cursor_target);

        cursor_target
    }

    fn draw_target_coordinate(&mut self) -> i32 {
        let span: u32 = WORLD_SIZE_PIXELS - 2 * CURSOR_TARGET_MARGIN_PIXELS;

        i32::try_from(CURSOR_TARGET_MARGIN_PIXELS + self.script_rng.below(span)).unwrap()
    }

    /// Zero aims included, which press nothing.
    fn draw_aim(&mut self) -> AimVector {
        let x: i32 = i32::try_from(self.script_rng.below(2 * AIM_EXTENT_PIXELS + 1)).unwrap();
        let y: i32 = i32::try_from(self.script_rng.below(2 * AIM_EXTENT_PIXELS + 1)).unwrap();
        let extent: i32 = i32::try_from(AIM_EXTENT_PIXELS).unwrap();

        AimVector {
            x: i16::try_from(x - extent).unwrap(),
            y: i16::try_from(y - extent).unwrap(),
        }
    }
}

fn create_scripted_settings() -> GameSettings {
    GameSettings {
        title: String::from("Scripted game"),
        mode: GameModeKind::Survival,
        world_shape: WorldShapeKind::Rectangle,
        world_width_pixels: WORLD_SIZE_PIXELS,
        world_height_pixels: WORLD_SIZE_PIXELS,
        player_minimum: Some(4),
        player_cap: 16,
        team_count: None,
        leaderboard_length: 10,
    }
}

/// Bit 0 picks the first ability, bit 1 the second, bit 2 the third.
fn create_loadout(player_index: u32) -> Loadout {
    let appearance_index: usize = usize::try_from(player_index % 4).unwrap();

    Loadout {
        appearance: Appearance {
            color: COLORS[appearance_index],
            skin: SKINS[appearance_index],
        },
        first: if player_index & 1 == 0 {
            FirstAbilityKind::Extend
        } else {
            FirstAbilityKind::Compress
        },
        second: if player_index & 2 == 0 {
            SecondAbilityKind::Immortality
        } else {
            SecondAbilityKind::Freeze
        },
        third: if player_index & 4 == 0 {
            ThirdAbilityKind::Neutralize
        } else {
            ThirdAbilityKind::Toxin
        },
    }
}

fn create_joined_participant_events(member_id: MemberId) -> Vec<MemberEvent> {
    let loadout: Loadout = create_loadout(member_id.0);

    vec![
        MemberEvent::Joined {
            member_id,
            screen_name: format!("scripted player {}", member_id.0),
            role: MemberRoleKind::Participant,
            loadout: Some(loadout),
            team: None,
        },
        MemberEvent::SpawnRequested {
            member_id,
            loadout,
            team: None,
        },
    ]
}

fn get_member_events(state: &GameState, tick: Tick) -> Vec<MemberEvent> {
    match tick {
        Tick(1) => {
            let mut member_events: Vec<MemberEvent> = (0..SCRIPTED_PLAYER_COUNT)
                .flat_map(|player_index| create_joined_participant_events(MemberId(player_index)))
                .collect();
            member_events.push(MemberEvent::Joined {
                member_id: MemberId(SCRIPTED_PLAYER_COUNT),
                screen_name: String::from("scripted spectator"),
                role: MemberRoleKind::Spectator,
                loadout: None,
                team: None,
            });

            member_events
        }
        LEAVE_TICK => vec![MemberEvent::Left {
            member_id: LEAVING_MEMBER_ID,
        }],
        REJOIN_TICK => create_joined_participant_events(state.next_member_id),
        APPEARANCE_CHANGE_TICK => vec![MemberEvent::AppearanceChanged {
            member_id: APPEARANCE_CHANGING_MEMBER_ID,
            appearance: Appearance {
                color: OrganismColorKind::Hot,
                skin: SkinKind::Ghost,
            },
        }],
        _ => Vec::new(),
    }
}

/// The checksum of the state after each tick.
fn run_scripted_game() -> Vec<u64> {
    let mut state: GameState = GameState::new(create_scripted_settings(), GAME_SEED);
    let mut scripted_players: ScriptedPlayers = ScriptedPlayers::new();
    let mut checksums: Vec<u64> = Vec::new();

    for _ in 0..SCRIPTED_TICK_COUNT {
        let bundle: InputBundle = scripted_players.create_bundle(&state);
        game::step(&mut state, &bundle).unwrap();
        checksums.push(state_checksum::get_state_checksum(&state));
    }

    checksums
}

fn get_checksum_fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scripted_game.checksums")
}

/// One line per tick: the tick, then the checksum in 16 hexadecimal digits.
fn format_checksums(checksums: &[u64]) -> String {
    checksums.iter().zip(1_u32..).map(|(checksum, tick)| format!("{tick} {checksum:016x}\n")).collect()
}

#[test]
fn step_gives_identical_checksums_in_two_runs() {
    assert_eq!(run_scripted_game(), run_scripted_game());
}

#[test]
fn step_matches_the_golden_checksum_sequence() {
    let golden_checksums: String = fs::read_to_string(get_checksum_fixture_path()).unwrap();

    assert_eq!(format_checksums(&run_scripted_game()), golden_checksums);
}

#[test]
#[ignore = "rewrites the golden checksums; ./scripts/test/regenerate-replay-fixtures.sh"]
fn regenerate_scripted_game_checksums() {
    let fixture_path: PathBuf = get_checksum_fixture_path();
    fs::create_dir_all(fixture_path.parent().unwrap()).unwrap();
    fs::write(fixture_path, format_checksums(&run_scripted_game())).unwrap();
}
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p shared --test determinism`
Expected: `test result: FAILED. 1 passed; 1 failed`; the failing tests are `step_matches_the_golden_checksum_sequence`.

- [ ] **Step 3: Implement**

Create `scripts/test/regenerate-replay-fixtures.sh`:

```sh
#!/usr/bin/env bash
# Rewrites the golden checksums of the scripted game from the current simulation.

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

cargo test -p shared --test determinism -- --ignored --exact regenerate_scripted_game_checksums
```

Make it executable: `chmod +x scripts/test/regenerate-replay-fixtures.sh`.

- [ ] **Step 4: Run `./scripts/test/regenerate-replay-fixtures.sh`**

Run: `./scripts/test/regenerate-replay-fixtures.sh`
Expected: `test regenerate_scripted_game_checksums ... ok`. Then `head -n 3 shared/tests/fixtures/scripted_game.checksums` prints

```text
1 c6dbd9a8567ee3e6
2 ad442d872a38a5aa
3 14c594ec7365cb35
```

and `tail -n 1 shared/tests/fixtures/scripted_game.checksums` prints `10000 56aaced17949ff09`. Different values mean the code differs from this plan somewhere; find the difference before committing the fixture.

- [ ] **Step 5: Run the tests to see them pass**

Run: `cargo test -p shared --test determinism`
Expected: no warnings; `test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 6: Commit**

```sh
git add shared/tests/determinism.rs scripts/test/regenerate-replay-fixtures.sh shared/tests/fixtures/scripted_game.checksums
git status --short
git commit -m "scripted game determinism tests"
```

Expected: before the commit, `git status --short` shows exactly the paths just added as staged (`A` or `M` in the first column) and no other staged or modified path.


### Task 27: Phase completion check

**Files:** none (verification only; no commit).

- [ ] **Step 1: Formatting**

Run: `cargo fmt --all -- --check`
Expected: no output, exit status 0.

- [ ] **Step 2: Build and test the workspace**

Run: `cargo build --workspace && cargo test --workspace; echo "exit=$?"`
Expected: no warnings; the `shared` library tests report `test result: ok. 225 passed; 0 failed; 0 ignored`, `determinism` reports `2 passed; 0 failed; 1 ignored`, `forbidden_operations` reports `3 passed`, `growth_statistics` reports `0 passed; 0 failed; 1 ignored`, every other `test result:` line is `ok`, then `exit=0`.

- [ ] **Step 3: wasm32 type check**

Run: `./scripts/test/check-wasm.sh; echo "exit=$?"`
Expected: the three checks end with `Finished`, then `exit=0`.

- [ ] **Step 4: Growth statistics**

Run: `./scripts/test/test-growth-statistics.sh; echo "exit=$?"`
Expected: `test result: ok. 1 passed`, then `exit=0`.

- [ ] **Step 5: Confirm the tree**

Run: `git status --short --untracked-files=no`
Expected: no output.

Run: `git log --oneline --reverse $(git log --grep '^>>> branch: simulation-abilities' --format=%h)..HEAD`
Expected, oldest first (hashes vary; review-fix commits, if any, sit after their task):

```text
<hash> phase 3 plan
<hash> tick arithmetic and millisecond conversion
<hash> ability durations, cooldowns and speeds
<hash> ability kind durations, shot effects and press helpers
<hash> ability phase expiry
<hash> field and secretion containment
<hash> organism centroid
<hash> spore launch
<hash> shot cell selection and launch
<hash> projectile flight
<hash> timer expiry phase
<hash> self-cast ability presses
<hash> shot slots and their effects
<hash> damage phase
<hash> step runs the ability phases
<hash> force spawn
<hash> round transitions and countdown
<hash> step runs survival shrinking and round transitions
<hash> game teams and team balance
<hash> leaderboard rows
<hash> ability icons
<hash> geometry wire types
<hash> organism wire types
<hash> member wire types
<hash> game state wire types
<hash> state checksum
<hash> scripted game determinism tests
```

## Roadmap coverage

- `ability`:
  - activation: Task 3 (press helpers, kind durations), Task 11 (extend, immortality, neutralize and toxin centred on the cursor, spore launch and secretion, member order), Task 12 (shot slots, pops, compress and freeze effects, key priority of the carried ability, miss, teammates, target's own neutralize);
  - projectiles: Task 6 (centroid), Task 7 (spores with the progressive centroid, dropped zero-offset spore, last spore), Task 8 (shot selection with wraparound, ties and zero aim), Task 9 (flight distance);
  - damage: Task 13 (spore and shot acid, self acid, toxin owner skip, neutralize protection, teammate immunity, last hitter);
  - constants: Tasks 1 and 2; timer expiry: Tasks 4 and 10; icons: Task 20.
- `round`: Task 16 (every transition, cancel, force spawn, survivor win, no winner, world restore, countdown), with force spawn and its cap in Task 15 and shrinking in Task 16.
- `member/scoreboard`: Task 19 (ordering per mode, length cut, team rows, K:D text). `member/team_assignment`: Task 18 (balance rule, smallest-team candidates, pending members).
- `simulation_event`: `EffectApplied` in Task 12, `RoundPhaseChanged` and `RoundWon` in Task 16.
- `checksum`: Tasks 21 to 25 (wire encoding of the state, FNV-1a 64).
- Complete tick order: Task 14 (phases 3, 4, 5 and 9) and Task 17 (phases 6 and 11) on top of phase 2's step.
- Determinism tests: Task 26 (10,000-tick scripted game with 16 players using every ability, stepped twice; golden checksum sequence).
- Kill credit cases of `testing.md` (last removal wins, suicide, departed hitter, teammate and neutralized hits record nothing): Task 13 together with phase 2's step tests.
- Deferred to phase 4, as `testing.md` places them on the protocol: the snapshot round trip, decoding and `TryFrom`, and the replay fixture.
