# Phase 9: verification implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** close the rewrite's verification: the replay fixture test under wasmtime (`wasm32-wasip1`), a recorded run of the growth statistics script, the manual verification list of `docs/architecture/testing.md`, a walk-through of `docs/architecture/bug-fix-ledger.md` against the code and its tests, and a documentation pass; done when every automated suite passes and the manual checks are recorded below for the PR.

**Architecture:** `docs/architecture/testing.md` (determinism tests, growth statistics, manual verification) and `docs/architecture/dev-workflow.md` (scripts). One new script runs the existing `shared/tests/replay.rs` built for `wasm32-wasip1` with wasmtime as Cargo's runner; the shared program check of the test scripts moves into a sourced file; the ledger walk-through adds a regression test wherever an original bug had no test of its own; everything else is observation, recorded in this plan's verification notes.

**Tech Stack:** Rust 1.95 (edition 2024), wasmtime 49 and the `wasm32-wasip1` target (approved test tooling, `overview.md`), bash, cargo-leptos, the chrome-devtools MCP tools; no new dependencies.

---

## Ground rules for the executor

- Repository `/Users/singularity/bacter-rs`, branch `verification`. Run `git -C /Users/singularity/bacter-rs branch --show-current` first; stop if it does not print `verification`.
- Every command runs from the repository root `/Users/singularity/bacter-rs`.
- Never switch branches, push, rebase or amend. Stage by explicit path, run `git status` before each commit, one-line commit messages without attribution. `?? .idea/` (the owner's IDE folder) stays untracked; it is left out of the expected `git status` listings below.
- Never run `cargo clippy`.
- The code below is `rustfmt`-formatted with the repository's `rustfmt.toml`; transcribe it exactly, so `cargo fmt --all -- --check` stays silent.
- Edits take these forms:
  - "Create `path`" with the whole new file;
  - "Replace the whole contents of `path`";
  - "In `path`, replace" a text which occurs exactly once in the file, "with" its replacement;
  - "In `path`, insert after" a text which occurs exactly once in the file;
  - "In `path`, insert before" a text which occurs exactly once in the file.
- Builds stay warning-free, tests included; a warning is a defect to fix before committing.
- Expected outputs were observed by executing Tasks 1 to 5 and the automated suites in a scratch clone of this branch. The `finished in` timing of each `test result` line, the build timings and `Compiling` lines are left out; a native panic line's thread number (`(61298604)`) differs per run.
- Sandboxes: commands which bind a loopback port (the server's integration tests, `./scripts/dev/server.sh`, `./scripts/dev/web.sh`, `wasm-pack test`) fail with `Operation not permitted (os error 1)` inside a command sandbox; run them outside it.
- Long commands (a cold `cargo test --workspace`, `./scripts/test/check-wasm.sh`, `./scripts/test/test-slow-consumer.sh`, `cargo leptos watch`) start with the Bash tool's `run_in_background` option and are polled with short commands.
- A ledger regression test (Tasks 3 to 5) guards behaviour the code already has, so it passes when written. Its failing run is a mutation check instead: a one-line edit removes the behaviour the entry's resolution provides (for Task 5 the neutralize protection itself, since the rewrite has one neutralize check for every acid source), the new test fails, and the edit is undone before the commit.
- Browser checks (Task 9): start `./scripts/dev/server.sh` and `./scripts/dev/web.sh` detached with `nohup` (a tracked background task could not bind 127.0.0.1:3011 in phase 8), drive Chrome through the chrome-devtools MCP tools (load them with ToolSearch, query `chrome-devtools navigate screenshot console`, or `select:` the exact names), and stop every process you started by its PID. Leave processes you did not start alone, including a `cargo leptos watch` of another project. If the MCP tools are unavailable, record that in the verification notes rather than claiming a check passed.
- The menus' radio buttons are `div`s the accessibility snapshot does not list; click them from `evaluate_script` (`document.querySelectorAll('.menu-radio')[index].click()`), counting them in row order (Skin 0 to 2, 1st ability 3 and 4, 2nd 5 and 6, 3rd 7 and 8, 4th 9, then Auto Assign).
- Results of every observation go into [Verification notes](#verification-notes) at the end of this file, one bullet per check, as the tasks say; the owner reads them in the PR.
- This file changes only where a task says so (verification notes, deviations); checkbox progress is tracked outside it, so the `git diff --stat` of Tasks 3 to 5 lists only the source file.

## Decisions taken where the design is silent or contradictory

- The growth statistics test and `scripts/test/test-growth-statistics.sh` already exist (phase 2 created them, since its done condition needed the test). Phase 9 runs the script and records the measured means against the reference table; nothing is rebuilt.
- `scripts/test/test-replay-wasm.sh` sets `CARGO_TARGET_WASM32_WASIP1_RUNNER` to `wasmtime run --dir=<repository>/shared/tests/fixtures`: the test locates its fixtures through `env!("CARGO_MANIFEST_DIR")`, an absolute host path, and wasmtime's `--dir` without a guest path preopens the directory under that same path. No `.cargo/config.toml`: the runner needs the repository's absolute path, and only this script runs the target.
- The wasm run uses the debug profile, so overflow checks stay on for the 32-bit `usize` arithmetic it exists to cover; it takes about 10 s.
- The wasm run passes `--nocapture`: under `wasm32-wasip1` a panic aborts the process, and libtest's captured output (the assertion message) would be lost.
- `require_program` moves into `scripts/test/test-preconditions.sh`, sourced by `test-wasm.sh` and `test-replay-wasm.sh`, as `scripts/git/git-preconditions.sh` is for the git scripts. `setup.sh` and `scripts/build/build-site.sh` keep their own copies: they sit outside `scripts/test/` (and `setup.sh` also needs `recommend_program`). `require_rust_target` has one consumer and stays in `test-replay-wasm.sh`.
- Ledger walk-through: an entry whose resolution is testable gets a regression test when no test exercises the original's specific defect. Three entries qualify: the spawn search stopping after the first organism, the round win applied more than once, and the neutralize check of shot secretions (the existing damage test uses a spore). Every other entry maps to an existing test or to a structural resolution (a type, a missing message, the tick order); the mapping is [Bug-fix ledger walk-through](#bug-fix-ledger-walk-through) below, checked mechanically in Task 6.
- Visual parity is judged against `docs/architecture/client-web.md` and the original's sources (`public/css/*.css`, `public/js/ui/*.js` of `/Users/singularity/bacter`, branch `master`): the original needs its Node and socket.io server, which this phase does not run.
- Safari is not driven: the chrome-devtools tools drive Chrome only, and `safaridriver` needs the owner to enable remote automation with administrator rights. The Safari checks are recorded as left to the owner.
- Network throttling uses the MCP `emulate` presets, which shape latency and throughput but cannot drop packets; packet loss is recorded as not emulated. CPU throttling (`cpuThrottlingRate`) drives the backlog resync path, which a slow network alone may not reach.
- The documentation pass is mechanical where it can be: every path the architecture and convention docs name must exist, and every source, test and script file must appear in an architecture doc. The pass found three files missing from the module trees (`shared/src/game/tick.rs`, `shared/src/game/test_fixture.rs`, `server/src/game/test_fixture.rs`), three named nowhere (`scripts/git/git-preconditions.sh`, `shared/tests/determinism.rs`, `shared/tests/helpers/replay_fixture.rs`) and one named script not yet present (`scripts/test/test-replay-wasm.sh`, created in Task 2).

## File structure

- Create `scripts/test/test-preconditions.sh`: `require_program`, sourced by the test scripts.
- Modify `scripts/test/test-wasm.sh`: source `test-preconditions.sh` in place of its own `require_program`.
- Create `scripts/test/test-replay-wasm.sh`: the replay fixture test for `wasm32-wasip1` under wasmtime.
- Modify `shared/src/organism/spawn.rs`: test `is_spawn_position_valid_checks_every_organism_after_the_first`.
- Modify `shared/src/round/round.rs`: test `run_round_transition_phase_gives_the_win_once_while_the_survivor_lives_on`.
- Modify `shared/src/ability/damage.rs`: test `run_damage_phase_spares_cells_inside_any_neutralize_field_from_a_shot_secretion`.
- Modify `docs/architecture/overview.md`: the three missing files in the `shared` and `server` module trees.
- Modify `docs/architecture/dev-workflow.md`: `scripts/test/test-preconditions.sh` and `scripts/git/git-preconditions.sh` in the script list.
- Modify `docs/architecture/testing.md`: the new regression tests in the simulation unit test list; `shared/tests/determinism.rs` and `shared/tests/helpers/replay_fixture.rs` in the determinism and replay test lists.
- Modify `docs/plans/2026-10-12-phase-9-verification.md` (this file): verification notes, deviations.

### Task 1: Shared program check for the test scripts

**Files:**
- Create: `scripts/test/test-preconditions.sh`
- Modify: `scripts/test/test-wasm.sh`

- [ ] **Step 1: Run the current script with the program missing**

Run: `env PATH=/usr/bin:/bin ./scripts/test/test-wasm.sh; echo "exit=$?"`

Expected (the baseline the refactor must keep):

```text
error: the `wasm-pack` program is required; install it with: cargo install wasm-pack
exit=1
```

- [ ] **Step 2: Create the shared check**

Create `scripts/test/test-preconditions.sh` (not executable; it is sourced):

```sh
# Precondition checks shared by the scripts in scripts/test/; sourced, not run.

readonly MISSING_PRECONDITION_EXIT_STATUS=1

function require_program {
    local program_name="$1"
    local install_hint="$2"

    local program_path
    program_path=$(command -v "${program_name}" || true)

    if test -z "${program_path}"; then
        echo "error: the \`${program_name}\` program is required; install it with: ${install_hint}" >&2
        exit "${MISSING_PRECONDITION_EXIT_STATUS}"
    fi
}
```

- [ ] **Step 3: Source it from `test-wasm.sh`**

Replace the whole contents of `scripts/test/test-wasm.sh` (its mode stays `100755`):

```sh
#!/usr/bin/env bash
# Runs the web crate's #[wasm_bindgen_test] cases in headless Chrome.
# wasm-pack needs a package manifest, so this runs from web/.
#
# Extra arguments are passed to wasm-pack, for example:
#   ./scripts/test/test-wasm.sh -- load_returns

set -euo pipefail

source "$(dirname "$0")/test-preconditions.sh"

require_program "wasm-pack" "cargo install wasm-pack"
require_program "chromedriver" "a build matching the installed Chrome major version, on PATH"

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}/web"

exec wasm-pack test --headless --chrome --features hydrate "$@"
```

- [ ] **Step 4: Check the behaviour is unchanged**

Run: `bash -n scripts/test/test-preconditions.sh && bash -n scripts/test/test-wasm.sh && echo syntax-ok; env PATH=/usr/bin:/bin ./scripts/test/test-wasm.sh; echo "exit=$?"`

Expected:

```text
syntax-ok
error: the `wasm-pack` program is required; install it with: cargo install wasm-pack
exit=1
```

- [ ] **Step 5: Commit**

```sh
git add scripts/test/test-preconditions.sh
git add scripts/test/test-wasm.sh
git status --short
git commit -m "share the test scripts' program check"
```

Expected `git status --short` before the commit:

```text
A  scripts/test/test-preconditions.sh
M  scripts/test/test-wasm.sh
```

### Task 2: Replay test under wasmtime

`scripts/test/test-replay-wasm.sh` builds `shared/tests/replay.rs` for `wasm32-wasip1` and runs it under wasmtime against the committed checksum files (`testing.md`, determinism tests).

**Files:**
- Create: `scripts/test/test-replay-wasm.sh`

- [ ] **Step 1: Run the script before it exists**

Run: `./scripts/test/test-replay-wasm.sh; echo "exit=$?"`

Expected: FAIL, the shell reports `no such file or directory: ./scripts/test/test-replay-wasm.sh` (zsh) or `./scripts/test/test-replay-wasm.sh: No such file or directory` (bash), then `exit=127`.

- [ ] **Step 2: Create the script**

Create `scripts/test/test-replay-wasm.sh`:

```sh
#!/usr/bin/env bash
# Runs the replay fixture test built for wasm32-wasip1 under wasmtime, so a 32-bit usize and wasm float behaviour
# must reproduce the committed checksums.
#
# Extra arguments are passed to the test binary, for example:
#   ./scripts/test/test-replay-wasm.sh every_replay_fixture

set -euo pipefail

source "$(dirname "$0")/test-preconditions.sh"

readonly WASI_TARGET="wasm32-wasip1"
readonly FIXTURE_DIRECTORY="shared/tests/fixtures"

function require_rust_target {
    local target_name="$1"

    local installed_target
    installed_target=$(rustup target list --installed | grep -x "${target_name}" || true)

    if test -z "${installed_target}"; then
        echo "error: the \`${target_name}\` Rust target is required; install it with: rustup target add ${target_name}" >&2
        exit "${MISSING_PRECONDITION_EXIT_STATUS}"
    fi
}

require_program "wasmtime" "brew install wasmtime"
require_rust_target "${WASI_TARGET}"

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

# The test reads its fixtures by their absolute host path, so the guest sees that directory at the same path.
export CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime run --dir=${repository_root}/${FIXTURE_DIRECTORY}"

# A panic aborts the wasm process before libtest prints the output it captured.
cargo test -p shared --test replay --target "${WASI_TARGET}" -- --nocapture "$@"
```

Then: `chmod +x scripts/test/test-replay-wasm.sh`

- [ ] **Step 3: Run it and see it pass**

Run: `./scripts/test/test-replay-wasm.sh 2>&1 | grep -v Compiling`

Expected (about 10 s for the run itself):

```text
    Finished `test` profile [unoptimized + debuginfo] target(s)
     Running tests/replay.rs (target/wasm32-wasip1/debug/deps/replay-<hash>.wasm)

running 2 tests
test every_replay_fixture_matches_its_checksums ... ok
test regenerate_scripted_game_fixture ... ignored, rewrites the scripted game fixture; ./scripts/test/regenerate-replay-fixtures.sh

test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

If `wasm32-wasip1` is missing, run `rustup target add wasm32-wasip1` (the ground rules allow it) and run again.

- [ ] **Step 4: See it fail on a wrong checksum**

Run: `sed -i '' 's/^10000 56aaced17949ff09$/10000 0000000000000000/' shared/tests/fixtures/scripted_game.checksums && ./scripts/test/test-replay-wasm.sh > /tmp/bacter-phase9-replay-failure.txt 2>&1; echo "exit=$?"; grep -E 'panicked|assertion|error: test failed' /tmp/bacter-phase9-replay-failure.txt; git checkout -- shared/tests/fixtures/scripted_game.checksums && git status --short shared/tests/fixtures`

Expected: the wasm process aborts on the assertion, the script fails, and the fixture is restored (the last `git status` prints nothing):

```text
exit=134
thread 'main' (1) panicked at shared/tests/replay.rs:288:9:
assertion `left == right` failed
error: test failed, to rerun pass `-p shared --test replay`
```

- [ ] **Step 5: Check the missing tool and target errors**

Run: `env PATH=/usr/bin:/bin ./scripts/test/test-replay-wasm.sh; echo "exit=$?"; mkdir -p /tmp/bacter-phase9-no-targets && printf '#!/usr/bin/env bash\necho aarch64-apple-darwin\n' > /tmp/bacter-phase9-no-targets/rustup && chmod +x /tmp/bacter-phase9-no-targets/rustup && env PATH="/tmp/bacter-phase9-no-targets:${PATH}" ./scripts/test/test-replay-wasm.sh; echo "exit=$?"`

Expected (the second run sees a `rustup` which lists no wasm target):

```text
error: the `wasmtime` program is required; install it with: brew install wasmtime
exit=1
error: the `wasm32-wasip1` Rust target is required; install it with: rustup target add wasm32-wasip1
exit=1
```

- [ ] **Step 6: Commit**

```sh
git add scripts/test/test-replay-wasm.sh
git status --short
git commit -m "replay test under wasmtime"
```

Expected `git status --short` before the commit:

```text
A  scripts/test/test-replay-wasm.sh
```

### Task 3: Spawn search checks every organism

Ledger entry "Spawn-position overlap check stops after the first org": the existing test places one organism, so it cannot tell "all organisms" from "the first".

**Files:**
- Test: `shared/src/organism/spawn.rs`

- [ ] **Step 1: Write the regression test**

In `shared/src/organism/spawn.rs`, insert after:

```rust
        assert!(is_spawn_position_valid(
            &state.world,
            &state.members,
            WorldPoint { x: 113, y: 106 },
        ));
    }
```

the text (one blank line first):

```rust

    #[test]
    fn is_spawn_position_valid_checks_every_organism_after_the_first() {
        let state: GameState = test_fixture::create_state_with_organisms(
            GameModeKind::FreeForAll,
            300,
            &[WorldPoint { x: 100, y: 100 }, WorldPoint { x: 200, y: 200 }],
        );

        assert!(!is_spawn_position_valid(
            &state.world,
            &state.members,
            WorldPoint { x: 200, y: 200 },
        ));
        assert!(is_spawn_position_valid(
            &state.world,
            &state.members,
            WorldPoint { x: 150, y: 150 },
        ));
    }
```

- [ ] **Step 2: Run it and see it pass**

Run: `cargo test -p shared --lib is_spawn_position_valid 2>&1 | grep -E '^test result|every_organism'`

Expected:

```text
test organism::spawn::tests::is_spawn_position_valid_checks_every_organism_after_the_first ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 462 filtered out
```

- [ ] **Step 3: Mutation check: restore the original's defect and see the test fail**

In `shared/src/organism/spawn.rs`, replace:

```rust
    members.values().all(|member| {
```

with:

```rust
    members.values().take(1).all(|member| {
```

Run: `cargo test -p shared --lib is_spawn_position_valid 2>&1 | grep -E '^test result|FAILED|panicked'`

Expected: FAIL, only the new test:

```text
test organism::spawn::tests::is_spawn_position_valid_checks_every_organism_after_the_first ... FAILED
thread 'organism::spawn::tests::is_spawn_position_valid_checks_every_organism_after_the_first' (<thread>) panicked at shared/src/organism/spawn.rs:349:9:
test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 462 filtered out
```

- [ ] **Step 4: Undo the mutation and see the test pass**

In `shared/src/organism/spawn.rs`, replace:

```rust
    members.values().take(1).all(|member| {
```

with:

```rust
    members.values().all(|member| {
```

Run: `cargo test -p shared --lib is_spawn_position_valid 2>&1 | grep -E '^test result' && git diff --stat`

Expected:

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 462 filtered out
 shared/src/organism/spawn.rs | 20 ++++++++++++++++++++
 1 file changed, 20 insertions(+)
```

- [ ] **Step 5: Commit**

```sh
git add shared/src/organism/spawn.rs
git status --short
git commit -m "spawn validity regression test past the first organism"
```

Expected `git status --short` before the commit:

```text
M  shared/src/organism/spawn.rs
```

### Task 4: One win per round

Ledger entry "'end round' and the win can be applied more than once": the existing survivor test steps only the tick that ends the round.

**Files:**
- Test: `shared/src/round/round.rs`

- [ ] **Step 1: Write the regression test**

In `shared/src/round/round.rs`, insert after:

```rust
        assert_eq!(state.members[&MemberId(2)].score.wins, 1);
    }
```

the text (one blank line first):

```rust

    #[test]
    fn run_round_transition_phase_gives_the_win_once_while_the_survivor_lives_on() {
        let mut state: GameState = create_survival_state(2, RoundPhase::Playing);
        give_organisms(&mut state, &[MemberId(1)]);

        let round_end_events: Vec<SimulationEvent> = run_round_transition_phase(&mut state, Tick(1001));
        let post_round_events: Vec<SimulationEvent> = run_round_transition_phase(&mut state, Tick(1002));

        assert_eq!(
            round_end_events,
            vec![
                SimulationEvent::RoundPhaseChanged {
                    phase: RoundPhase::PostRound,
                },
                SimulationEvent::RoundWon { member_id: MemberId(1) },
            ],
        );
        assert_eq!(post_round_events, Vec::new());
        assert_eq!(state.members[&MemberId(1)].score.wins, 1);
    }
```

- [ ] **Step 2: Run it and see it pass**

Run: `cargo test -p shared --lib run_round_transition_phase 2>&1 | grep -E '^test result|win_once'`

Expected:

```text
test round::round::tests::run_round_transition_phase_gives_the_win_once_while_the_survivor_lives_on ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 459 filtered out
```

- [ ] **Step 3: Mutation check: let the round end again in PostRound and see the test fail**

In `shared/src/round/round.rs`, replace:

```rust
        RoundPhase::Playing if state.alive_organism_count() <= 1 => end_round(state, tick),
```

with:

```rust
        RoundPhase::Playing | RoundPhase::PostRound if state.alive_organism_count() <= 1 => end_round(state, tick),
```

Run: `cargo test -p shared --lib run_round_transition_phase 2>&1 | grep -E '^test result|FAILED'`

Expected: FAIL; the new test and the post-round restore test (whose survivors are also ended again):

```text
test round::round::tests::run_round_transition_phase_gives_the_win_once_while_the_survivor_lives_on ... FAILED
test round::round::tests::run_round_transition_phase_restores_the_world_and_force_spawns_after_the_post_round ... FAILED
test result: FAILED. 8 passed; 2 failed; 0 ignored; 0 measured; 459 filtered out
```

The two `FAILED` lines may come in either order.

- [ ] **Step 4: Undo the mutation and see the tests pass**

In `shared/src/round/round.rs`, replace:

```rust
        RoundPhase::Playing | RoundPhase::PostRound if state.alive_organism_count() <= 1 => end_round(state, tick),
```

with:

```rust
        RoundPhase::Playing if state.alive_organism_count() <= 1 => end_round(state, tick),
```

Run: `cargo test -p shared --lib run_round_transition_phase 2>&1 | grep -E '^test result' && git diff --stat`

Expected:

```text
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 459 filtered out
 shared/src/round/round.rs | 21 +++++++++++++++++++++
 1 file changed, 21 insertions(+)
```

- [ ] **Step 5: Commit**

```sh
git add shared/src/round/round.rs
git status --short
git commit -m "round win regression test for the post-round ticks"
```

Expected `git status --short` before the commit:

```text
M  shared/src/round/round.rs
```

### Task 5: Neutralize against shot secretions

Ledger entry "Shoot-secretion neutralize check indexes cells with the wrong loop variable": the existing neutralize damage test uses a spore secretion only.

**Files:**
- Test: `shared/src/ability/damage.rs`

- [ ] **Step 1: Write the regression test**

In `shared/src/ability/damage.rs`, insert before:

```rust
    #[test]
    fn run_damage_phase_spares_teammates_of_the_owner() {
```

the text (it ends with one blank line):

```rust
    #[test]
    fn run_damage_phase_spares_cells_inside_any_neutralize_field_from_a_shot_secretion() {
        let mut state: GameState = create_state(GameModeKind::FreeForAll);
        secrete_shot_at(&mut state, OWNER_ID, 0, VICTIM_POSITION);
        activate_field(
            &mut state,
            OTHER_OWNER_ID,
            ThirdAbilityKind::Neutralize,
            WorldPoint { x: 245, y: 300 },
        );

        run_damage_phase(&mut state);

        assert_eq!(get_cells(&state, VICTIM_ID), vec![LatticeCoordinate { i: 0, j: 0 }]);
    }

```

- [ ] **Step 2: Run it and see it pass**

Run: `cargo test -p shared --lib run_damage_phase_spares 2>&1 | grep -E '^test result|shot_secretion'`

Expected:

```text
test ability::damage::tests::run_damage_phase_spares_cells_inside_any_neutralize_field_from_a_shot_secretion ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 467 filtered out
```

- [ ] **Step 3: Mutation check: drop the neutralize protection and see the test fail**

In `shared/src/ability/damage.rs`, replace:

```rust
            let is_neutralized: bool = is_protected_by_neutralize(neutralize_field_centers, cell_center);
```

with:

```rust
            let is_neutralized: bool = false && is_protected_by_neutralize(neutralize_field_centers, cell_center);
```

Run: `cargo test -p shared --lib run_damage_phase_spares 2>&1 | grep -E '^test result|FAILED'`

Expected: FAIL; both neutralize tests, in either order:

```text
test ability::damage::tests::run_damage_phase_spares_cells_inside_any_neutralize_field ... FAILED
test ability::damage::tests::run_damage_phase_spares_cells_inside_any_neutralize_field_from_a_shot_secretion ... FAILED
test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 467 filtered out
```

- [ ] **Step 4: Undo the mutation and see the tests pass**

In `shared/src/ability/damage.rs`, replace:

```rust
            let is_neutralized: bool = false && is_protected_by_neutralize(neutralize_field_centers, cell_center);
```

with:

```rust
            let is_neutralized: bool = is_protected_by_neutralize(neutralize_field_centers, cell_center);
```

Run: `cargo test -p shared --lib run_damage_phase_spares 2>&1 | grep -E '^test result' && cargo fmt --all -- --check && git diff --stat`

Expected:

```text
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 467 filtered out
 shared/src/ability/damage.rs | 16 ++++++++++++++++
 1 file changed, 16 insertions(+)
```

- [ ] **Step 5: Commit**

```sh
git add shared/src/ability/damage.rs
git status --short
git commit -m "neutralize regression test for shot secretions"
```

Expected `git status --short` before the commit:

```text
M  shared/src/ability/damage.rs
```

### Task 6: Bug-fix ledger walk-through

Every entry of `docs/architecture/bug-fix-ledger.md` is matched in [Bug-fix ledger walk-through](#bug-fix-ledger-walk-through) below to the tests which exercise its resolution, or to the structural fact which makes the original's defect impossible. This task checks the mapping mechanically and reads the structural claims against the code.

**Files:**
- Modify: `docs/plans/2026-10-12-phase-9-verification.md` (verification notes)

- [ ] **Step 1: Write the mapping check**

Create `/tmp/bacter-phase9-check-ledger-tests.sh` (a throwaway script, not part of the repository):

```sh
#!/usr/bin/env bash
# Every test the walk-through names must exist exactly once and carry a test attribute.

set -euo pipefail

readonly PLAN_PATH="docs/plans/2026-10-12-phase-9-verification.md"

cd /Users/singularity/bacter-rs

walk_through=$(awk '/^## Bug-fix ledger walk-through$/ { inside = 1; next } /^## / { inside = 0 } inside' "${PLAN_PATH}")
test_names=$(echo "${walk_through}" | grep -o '`[a-z][a-z0-9_]*`' | tr -d '`' | grep -E '^([a-z0-9]+_){3,}[a-z0-9]+$' | sort -u)

name_count=0
problem_count=0

for test_name in ${test_names}; do
    name_count=$((name_count + 1))
    definition_files=$(grep -rl --include='*.rs' "fn ${test_name}(" shared server web tools || true)
    definition_count=$(echo "${definition_files}" | grep -c . || true)

    if test "${definition_count}" -ne 1; then
        echo "not exactly one definition: ${test_name} (${definition_count})"
        problem_count=$((problem_count + 1))
        continue
    fi

    test_attribute=$(grep -B3 "fn ${test_name}(" ${definition_files} | grep -E '#\[(test|tokio::test|actix_web::test)' || true)

    if test -z "${test_attribute}"; then
        echo "not a test: ${test_name}"
        problem_count=$((problem_count + 1))
    fi
done

echo "${name_count} tests named, ${problem_count} problems"
```

- [ ] **Step 2: Run it**

Run: `bash /tmp/bacter-phase9-check-ledger-tests.sh`

Expected:

```text
98 tests named, 0 problems
```

The named tests run in Task 10's `cargo test --workspace`; a name which fails there is a ledger problem as well.

- [ ] **Step 3: Read the structural resolutions against the code**

For each bullet of the walk-through marked "structural", open the code it names and confirm the claim; the checks with commands:

- No `alert(` in the web client: `grep -rn 'alert(' web/src` prints nothing.
- No client message replaces state, a password or a game's title after creation, and none ends a game: `sed -n 14,46p shared/src/protocol/message_in.rs` shows `MessageSerialIn`'s variants (`SubscribeGameList` to `LeaveGame`); none of them carries a board, a leaderboard, a member list, a password change or an end-game request.
- The tick order applies presses before births and deaths: `sed -n 15,48p shared/src/game/step.rs` shows `run_ability_press_phase` before `run_birth_phase`.
- A spawned organism starts with fresh abilities: `grep -n 'Organism::new' shared/src/organism/spawn.rs` shows both spawn paths constructing a new `Organism`, whose `abilities` are `OrganismAbilities::all_ready()` (`shared/src/organism/organism.rs`).
- One socket at a time, no automatic reconnect: `grep -n 'connect()' web/src/canvas/driver.rs` shows `connect()` called once at start (after the driver is created) and from `reconnect`, which runs only for `DriverCommandKind::Reconnect` and only while no socket is open; `GameSocket::open` is called only from `open_socket`, which `connect` reaches.

- [ ] **Step 4: Record the result**

Append to [Verification notes](#verification-notes), under `### Bug-fix ledger walk-through`, one bullet with the script's output line and one bullet per structural check with what the command showed; name any entry whose claim did not hold, and do not change the code for it in this task (report it under the deviations in Task 10).

- [ ] **Step 5: Commit**

```sh
git add docs/plans/2026-10-12-phase-9-verification.md
git status --short
git commit -m "bug-fix ledger walk-through notes"
```

Expected `git status --short` before the commit:

```text
M  docs/plans/2026-10-12-phase-9-verification.md
```

### Task 7: Growth statistics

**Files:**
- Modify: `docs/plans/2026-10-12-phase-9-verification.md` (verification notes)

- [ ] **Step 1: Run the script**

Run: `./scripts/test/test-growth-statistics.sh 2>&1 | grep -v Compiling`

Expected: PASS, each state's means within 10% (cells) and 15% (changes per tick) of the reference table in `testing.md`; the planning run printed:

```text
running 1 test
Default Still: cells 64.0 (reference 64.0), changes per tick 6.22 (reference 6.2)
Default Moving: cells 42.6 (reference 42.5), changes per tick 7.78 (reference 7.7)
Compress Still: cells 37.7 (reference 37.8), changes per tick 0.79 (reference 0.8)
Compress Moving: cells 23.5 (reference 23.0), changes per tick 4.46 (reference 4.4)
Extend Still: cells 124.1 (reference 124.8), changes per tick 7.97 (reference 7.8)
Extend Moving: cells 82.3 (reference 81.2), changes per tick 10.42 (reference 10.4)
test growth_statistics_match_the_original_rules ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The run is seeded, so the same simulation prints the same numbers.

- [ ] **Step 2: Record the result**

Append to [Verification notes](#verification-notes), under `### Growth statistics`, the six measured lines as printed and the `test result` line.

- [ ] **Step 3: Commit**

```sh
git add docs/plans/2026-10-12-phase-9-verification.md
git status --short
git commit -m "growth statistics verification notes"
```

Expected `git status --short` before the commit:

```text
M  docs/plans/2026-10-12-phase-9-verification.md
```

### Task 8: Documentation pass

The architecture keeps describing the code: the module trees gain the files they miss, the script list gains the sourced test preconditions, and the simulation test list names the new regression tests.

**Files:**
- Modify: `docs/architecture/overview.md`
- Modify: `docs/architecture/dev-workflow.md`
- Modify: `docs/architecture/testing.md`

- [ ] **Step 1: Write the documentation check**

Create `/tmp/bacter-phase9-check-docs.sh` (throwaway):

```sh
#!/usr/bin/env bash
# Paths the docs name must exist; source files must appear in an architecture doc.

set -euo pipefail

cd /Users/singularity/bacter-rs

readonly SEARCH_PREFIXES=("" shared/src/ server/src/ web/src/ shared/tests/ server/tests/ web/ docs/architecture/ docs/conventions/ scripts/test/ scripts/build/ scripts/dev/ scripts/git/ scripts/run/ web/static/)

# Names the docs use as examples of a pattern, not as files.
readonly ILLUSTRATIVE_NAMES="types.rs x_db.rs x_model.rs x/x.rs account_db.rs"

missing_count=0

for doc_path in docs/architecture/*.md docs/conventions/*.md CLAUDE.md; do
    doc_tokens=$(grep -o '`[^`]*`' "${doc_path}" | tr -d '`' | grep -E '^[A-Za-z0-9_./-]+\.(rs|sh|wgsl|json|scss|ttf)$' | sort -u || true)

    for doc_token in ${doc_tokens}; do
        is_illustrative=$(echo " ${ILLUSTRATIVE_NAMES} " | grep -F " ${doc_token} " || true)

        if test -n "${is_illustrative}"; then
            continue
        fi

        found_path=""

        for search_prefix in "${SEARCH_PREFIXES[@]}"; do
            if test -e "${search_prefix}${doc_token}"; then
                found_path="${search_prefix}${doc_token}"
                break
            fi
        done

        if test -z "${found_path}"; then
            base_name=$(basename "${doc_token}")
            found_path=$(find shared server web tools scripts -path '*/target' -prune -o -name "${base_name}" -print | head -1)
        fi

        if test -z "${found_path}"; then
            echo "named but missing: ${doc_path}: ${doc_token}"
            missing_count=$((missing_count + 1))
        fi
    done
done

source_paths=$(find shared/src server/src web/src shared/tests server/tests web/tests scripts -name '*.rs' -o -name '*.sh' | grep -v -e '/mod.rs$' -e '/lib.rs$' -e '/main.rs$' -e '/fixtures/' | sort)

for source_path in ${source_paths}; do
    base_name=$(basename "${source_path}")
    mention=$(grep -l -F "${base_name}" docs/architecture/*.md || true)

    if test -z "${mention}"; then
        echo "undocumented: ${source_path}"
        missing_count=$((missing_count + 1))
    fi
done

echo "${missing_count} problems"
```

- [ ] **Step 2: Run it and see it fail**

Run: `bash /tmp/bacter-phase9-check-docs.sh`

Expected: FAIL, the files missing from the module trees and the script and test lists (the replay script exists since Task 2):

```text
undocumented: scripts/git/git-preconditions.sh
undocumented: scripts/test/test-preconditions.sh
undocumented: shared/src/game/tick.rs
undocumented: shared/tests/determinism.rs
undocumented: shared/tests/helpers/replay_fixture.rs
5 problems
```

`shared/src/game/test_fixture.rs` and `server/src/game/test_fixture.rs` pass this check only through the web tree's `test_fixture.rs` line; Step 3 lists them in their own trees.

- [ ] **Step 3: Fill the module trees**

In `docs/architecture/overview.md`, replace:

```text
                  simulation_event.rs    SimulationEvent, SpawnRejectionKind
```

with:

```text
                  simulation_event.rs    SimulationEvent, SpawnRejectionKind
                  tick.rs                Tick, TICK_PERIOD_MILLISECONDS, millisecond and second conversions
                  test_fixture.rs        (test, feature test-support) settings, members and states for tests
```

In `docs/architecture/overview.md`, replace:

```text
             replay_capture.rs    optional replay record stream, writer task
```

with:

```text
             replay_capture.rs    optional replay record stream, writer task
             test_fixture.rs      (test) settings, joiners, members, game handles and temporary directories
```

- [ ] **Step 4: List the sourced preconditions**

In `docs/architecture/dev-workflow.md`, replace:

```markdown
- `scripts/test/test-replay-wasm.sh`: [determinism tests](./testing.md#determinism-tests).
```

with:

```markdown
- `scripts/test/test-replay-wasm.sh`: [determinism tests](./testing.md#determinism-tests).
- `scripts/test/test-preconditions.sh`: `require_program`; sourced, not run directly.
```

In `docs/architecture/dev-workflow.md`, insert after:

```markdown
- `scripts/git/branch-init.sh`, `pr-integrate.sh`, `cleanup-merged.sh`: adapted from eafora; `pr-integrate.sh` has no `--budget` mode (bacter-rs has no site budget script).
```

the line:

```markdown
- `scripts/git/git-preconditions.sh`: the git scripts' shared checks; sourced, not run directly.
```

- [ ] **Step 5: Name the regression tests and the shared test files**

In `docs/architecture/testing.md`, replace:

```markdown
- Rounds: every transition, cancel, force spawn cap, survivor win, simultaneous deaths with no winner, world restore.
- Spawn: hazards, attempt limit.
```

with:

```markdown
- Rounds: every transition, cancel, force spawn cap, survivor win (once per round), simultaneous deaths with no winner, world restore.
- Spawn: hazards, every organism checked, attempt limit.
```

In `docs/architecture/testing.md`, replace:

```markdown
pop applies once per target, neutralize protection, teammate immunity,
```

with:

```markdown
pop applies once per target, neutralize protection against spore and shot acid, teammate immunity,
```

In `docs/architecture/testing.md`, replace:

```markdown
- Same seed and bundles stepped twice in process give
```

with:

```markdown
- `shared/tests/determinism.rs`: same seed and bundles stepped twice in process give
```

In `docs/architecture/testing.md`, replace:

```markdown
- Snapshot round trip: at tick t,
```

with:

```markdown
- Snapshot round trip (`shared/tests/determinism.rs`): at tick t,
```

In `docs/architecture/testing.md`, insert after:

```markdown
- `shared/tests/replay.rs`: every fixture in `shared/tests/fixtures/*.replay` re-run, compared with its committed checksum file.
```

the line:

```markdown
- `shared/tests/helpers/replay_fixture.rs`: reads `scripted_game.replay` into a `ReplayLog`.
```

- [ ] **Step 6: Run the check and see it pass**

Run: `bash /tmp/bacter-phase9-check-docs.sh && grep -c 'tick.rs\|test_fixture.rs\|test-preconditions.sh\|git-preconditions.sh\|determinism.rs\|replay_fixture.rs\|once per round\|every organism checked\|spore and shot acid' docs/architecture/overview.md docs/architecture/dev-workflow.md docs/architecture/testing.md`

Expected (the three count lines in any order):

```text
0 problems
docs/architecture/overview.md:4
docs/architecture/dev-workflow.md:2
docs/architecture/testing.md:6
```

`overview.md` counts the two new `test_fixture.rs` lines, the `tick.rs` line and the web tree's existing `test_fixture/ test_fixture.rs` line; `dev-workflow.md` the two precondition lines; `testing.md` the ability, rounds and spawn lines, the two `determinism.rs` lines and the `replay_fixture.rs` line.

- [ ] **Step 7: Commit**

```sh
git add docs/architecture/overview.md
git add docs/architecture/dev-workflow.md
git add docs/architecture/testing.md
git status --short
git commit -m "documentation pass for phase 9"
```

Expected `git status --short` before the commit:

```text
M  docs/architecture/dev-workflow.md
M  docs/architecture/overview.md
M  docs/architecture/testing.md
```

### Task 9: Manual verification in the browser

The manual verification list of `testing.md`: visual parity with the original, both renderer backends at device pixel ratios 1 and 2, feel, and two browsers in one game under throttling. UI views are verified manually (`docs/conventions/testing.md`); every result is recorded.

**Files:**
- Modify: `docs/plans/2026-10-12-phase-9-verification.md` (verification notes)

- [ ] **Step 1: Check the ports and start the servers**

Run: `lsof -nP -iTCP:3011 -iTCP:3012 -iTCP:3100 -sTCP:LISTEN`

Expected: no output. A listener on any of these ports belongs to someone else: record it in the notes, leave it running, and stop this task (the dev servers cannot bind).

Run (outside a sandbox): `nohup ./scripts/dev/server.sh > /tmp/bacter-phase9-server.log 2>&1 & echo "server pid $!"; nohup ./scripts/dev/web.sh > /tmp/bacter-phase9-web.log 2>&1 & echo "web pid $!"`

Note both PIDs. Poll until both listen (the web build takes about a minute): `lsof -nP -iTCP:3100 -iTCP:3011 -sTCP:LISTEN; grep -m1 'Serving at' /tmp/bacter-phase9-web.log`

Expected: listeners on 3100 and 3011, and `Serving at http://127.0.0.1:3011` in the web log.

- [ ] **Step 2: Load the browser tools**

Load the chrome-devtools MCP tools with ToolSearch (`select:mcp__plugin_chrome-devtools-mcp_chrome-devtools__new_page,mcp__plugin_chrome-devtools-mcp_chrome-devtools__navigate_page,mcp__plugin_chrome-devtools-mcp_chrome-devtools__take_screenshot,mcp__plugin_chrome-devtools-mcp_chrome-devtools__take_snapshot,mcp__plugin_chrome-devtools-mcp_chrome-devtools__evaluate_script,mcp__plugin_chrome-devtools-mcp_chrome-devtools__click,mcp__plugin_chrome-devtools-mcp_chrome-devtools__fill,mcp__plugin_chrome-devtools-mcp_chrome-devtools__press_key,mcp__plugin_chrome-devtools-mcp_chrome-devtools__list_console_messages,mcp__plugin_chrome-devtools-mcp_chrome-devtools__emulate,mcp__plugin_chrome-devtools-mcp_chrome-devtools__get_css_styles,mcp__plugin_chrome-devtools-mcp_chrome-devtools__close_page`). Open `http://127.0.0.1:3011` with `new_page` and set `emulate` `viewport` to `1280x800x1`.

- [ ] **Step 3: Visual parity with the original**

Compare each screen with `docs/architecture/client-web.md` (Screens, Renderer, Overlays) and the original's `public/css/title.css`, `menus.css`, `browser.css`, `index.css`, `canvas.css` and `public/js/ui/` (`TitleMenu.js`, `Browser.js`, `HUD.js`, `Board.js`, `Message.js`, `World.js`, `menus/`) in `/Users/singularity/bacter` (read only). Read computed values with `get_css_styles` or `evaluate_script` (`getComputedStyle`), the canvas with `take_screenshot`. Check:

- Title: box 170×150, background rgb(10,10,10), 1 px white border, Host, Join and Tutorial in `_bacter` 29 px; the title scene's dummy organisms grow behind it in a black world inset 25 px with its shadow; the notice line below the box.
- Browser: the table's columns (Title, Mode, Players, Spectators, Player Cap, Join or Spectate), the "← Back" and "Online Clients: N" footer, against `browser.css`.
- Create, join, spectate, respawn and pause menus: header, two-column striped rows, red per-field errors, the bottom general row, the Back footer, against `menus.css`; the per-mode visibility of Create (ffa, skm, srv).
- In a game (create a Free for All game, world 600): the world colours and draw order of the Renderer section (backdrop 70,70,70, shadow, black body, 210 border), each skin (Grid, Circles, Ghost, None; join four times, or change it in the pause menu), the ability bar's slots, dials, icons, key tabs and dots against `HUD.js`, the leaderboard's place, row height, columns and own-row bold against `Board.js`, the message box's place and font against `Message.js`, the name label under the organism.
- Tutorial: the task texts of `public/js/ui/Message.js` (its tutorial branch) in the message box, the crosshair over a still world, the HUD hidden during the move and survive tasks.

Record one bullet per screen: parity, or each difference with the original's value and ours.

- [ ] **Step 4: Renderer backends and device pixel ratios in Chrome**

For each of the four combinations, `emulate` `viewport` `1280x800x1` or `1280x800x2`, then navigate to `http://127.0.0.1:3011` or `http://127.0.0.1:3011/?renderer=webgl2`:

- `list_console_messages`: one `renderer created; [adapter=...]` line whose backend is `BrowserWebGpu` (default) or `Gl` (`?renderer=webgl2`), and no error;
- `take_screenshot` of the title, then of a running game: edges crisp at ratio 2, the world, cells and crosshair at the same CSS-pixel size at both ratios, the ability bar at nine tenths of the height, the name label under its organism.

Record one bullet per combination. Safari: record "not run; the chrome-devtools tools drive Chrome only, and `safaridriver` needs the owner to enable remote automation" with the Safari checks (WebGPU and `?renderer=webgl2`, ratios 1 and 2) left to the owner.

- [ ] **Step 5: Feel**

At ratio 1, open the tutorial (Tutorial on the title) and measure the crosshair speed with `evaluate_script`:

```js
async () => {
  const canvas = document.querySelector('.game-canvas');
  const waitForFrame = () => new Promise((resolve) => requestAnimationFrame(() => resolve()));
  const findCrosshair = async () => {
    await waitForFrame();
    const image = new Image();
    image.src = canvas.toDataURL('image/png');
    await image.decode();
    const copyCanvas = document.createElement('canvas');
    copyCanvas.width = image.width;
    copyCanvas.height = image.height;
    const copyContext = copyCanvas.getContext('2d');
    copyContext.drawImage(image, 0, 0);
    const pixels = copyContext.getImageData(0, 0, image.width, image.height).data;
    let sumX = 0;
    let sumY = 0;
    let whitePixelCount = 0;
    for (let offset = 0; offset < pixels.length; offset += 4) {
      const isWhite = pixels[offset] >= 250 && pixels[offset + 1] >= 250 && pixels[offset + 2] >= 250;
      if (isWhite) {
        const pixelIndex = offset / 4;
        sumX += pixelIndex % image.width;
        sumY += Math.floor(pixelIndex / image.width);
        whitePixelCount += 1;
      }
    }
    return { x: sumX / whitePixelCount, y: sumY / whitePixelCount, whitePixelCount };
  };
  const before = await findCrosshair();
  const startMilliseconds = performance.now();
  window.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyD', key: 'd' }));
  await new Promise((resolve) => setTimeout(resolve, 2000));
  window.dispatchEvent(new KeyboardEvent('keyup', { code: 'KeyD', key: 'd' }));
  const heldMilliseconds = performance.now() - startMilliseconds;
  const after = await findCrosshair();
  const pixelsPerSecond = (after.x - before.x) / window.devicePixelRatio / (heldMilliseconds / 1000);
  return { before, after, heldMilliseconds, pixelsPerSecond };
}
```

Expected: `pixelsPerSecond` close to 42.5 (the original's crosshair speed), `after.y` equal to `before.y`. A `whitePixelCount` of 0 means the canvas copy came back empty: record that and estimate the movement from two `take_screenshot`s instead.

Then in an online Free for All game (the servers of Step 1), with a loadout of Extend, Freeze, Toxin: sample the fourth slot's dial every 100 ms for 11 s after a Space press, and the first slot's for 9 s after an X press, with `evaluate_script`:

```js
async () => {
  const slotIndex = 3;
  const keyCode = 'Space';
  const sampleCount = 110;
  const readDial = () => document.querySelectorAll('.ability-slot')[slotIndex].querySelector('.ability-dial-fill').getAttribute('d');
  window.dispatchEvent(new KeyboardEvent('keydown', { code: keyCode, key: ' ' }));
  window.dispatchEvent(new KeyboardEvent('keyup', { code: keyCode, key: ' ' }));
  const samples = [];
  for (let index = 0; index < sampleCount; index += 1) {
    samples.push(readDial());
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  const changeTimes = [];
  for (let index = 1; index < samples.length; index += 1) {
    if (samples[index] !== samples[index - 1]) {
      changeTimes.push(index * 100);
    }
  }
  return { first: samples[0], last: samples[samples.length - 1], changeTimes };
}
```

(for X: `slotIndex = 0`, `keyCode = 'KeyX'`, key `'x'`, `sampleCount = 90`). Expected timings from `ability_constants.rs`: the spore flies about 1.7 s, then cools 7.5 s (no secretion without a second press); Extend is active 4.5 s and cools 4 s; the dial drains while active or flying and refills while cooling. Watch a spore and a Freeze shot fly in `take_screenshot`s taken during the flight. Record the measured speed, the dial timings and what the flights looked like.

- [ ] **Step 6: Two browsers, then throttling**

Open a second page with `new_page` and `isolatedContext` set to `second-player`; join the game of Step 5 with another name. On both pages: both organisms, both leaderboard rows, both name labels, and no warning in `list_console_messages`.

On the second page, in turn, for about 30 s each while holding a movement key in bursts (`keydown` and `keyup` from `evaluate_script`):

- `emulate` `networkConditions` `Slow 3G`, then `Fast 3G`;
- `emulate` `cpuThrottlingRate` 20 (no network throttling);
- then `emulate` with `cpuThrottlingRate` 1 and no `networkConditions` (both off).

The client advances only from `requestAnimationFrame`, which Chrome pauses on a hidden page, so a hidden page builds a backlog of its own. Before each condition and before each `list_console_messages`, run `evaluate_script` `() => document.visibilityState` on both pages and record it. If a page reports `hidden`, bring it forward with `select_page` (`bringToFront`) or tile the two windows; attribute any warning from a page which was hidden to the hidden page, not to the throttle.

After each, `list_console_messages` on both pages. Record per condition: whether the game kept playing on both pages, any `applied tick too far behind the server`, `state checksum differs from the server's`, `synchronisation lost, leaving the game` or `cursor correction dropped` warning, whether the crosshair jumped back (a clamp), and whether the "Catching up" message showed during a resync. Record packet loss as not emulated (the presets shape latency and throughput only).

- [ ] **Step 7: Stop every process you started**

Close both pages (`close_page`). Then: `kill <server pid> <web pid>`, and stop what still listens on the dev ports, which are this task's processes (the ports were free in Step 1): `listener_ids=$(lsof -nP -t -iTCP:3011 -iTCP:3012 -iTCP:3100 -sTCP:LISTEN || true); test -z "${listener_ids}" || kill ${listener_ids}`. Run `lsof -nP -iTCP:3011 -iTCP:3012 -iTCP:3100 -sTCP:LISTEN`.

Expected: no output.

- [ ] **Step 8: Record and commit**

Append to [Verification notes](#verification-notes), under `### Manual checks`, the bullets of Steps 3 to 6, then:

```sh
git add docs/plans/2026-10-12-phase-9-verification.md
git status --short
git commit -m "manual verification notes"
```

Expected `git status --short` before the commit:

```text
M  docs/plans/2026-10-12-phase-9-verification.md
```

### Task 10: Phase verification

**Files:**
- Modify: `docs/plans/2026-10-12-phase-9-verification.md` (verification notes, deviations)

- [ ] **Step 1: Run the workspace tests**

Run (outside a sandbox; the server's integration tests bind loopback ports): `cargo test --workspace 2>&1 | grep -E '^test result|^test .*FAILED'`

Expected: every line `ok`, in this order:

```text
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 120 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 13 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 509 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 107 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

(`shared`'s unit tests: 506 before this phase plus the three regression tests.)

- [ ] **Step 2: Run the feature and ignored suites**

Run: `cargo test -p web --features ssr --test prerender 2>&1 | grep -E '^test result'; ./scripts/test/test-shaders.sh 2>&1 | grep -E '^test result'; ./scripts/test/test-slow-consumer.sh 2>&1 | grep -E '^test result'; ./scripts/test/test-growth-statistics.sh 2>&1 | grep -E '^test result'; ./scripts/test/test-replay-wasm.sh 2>&1 | grep -E '^test result'`

Expected (the slow consumer test takes about 45 s):

```text
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 509 filtered out
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 3: Check the browser builds and the formatting**

Run: `./scripts/test/check-wasm.sh 2>&1 | grep -E '^(warning|error)|Finished' && cargo fmt --all -- --check && echo formatted`

Expected:

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
    Finished `dev` profile [unoptimized + debuginfo] target(s)
formatted
```

- [ ] **Step 4: Run the browser tests if chromedriver matches Chrome**

Run: `chromedriver --version; "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --version`

If both major versions match (the planning run saw ChromeDriver 149 against Chrome 154), run `./scripts/test/test-wasm.sh 2>&1 | grep -E '^test result|FAILED|error'` (outside a sandbox) and expect `test result: ok.` with every `LocalStorageSettingsStore` test passed. Otherwise do not download a chromedriver: record the two versions and "not run" in the notes.

- [ ] **Step 5: Self-review**

Review the phase's diff (`git diff web-client --stat` and `git diff web-client -- scripts shared docs/architecture`) against `CLAUDE.md`, `docs/conventions/` and `~/.claude/CLAUDE.md`: script rules of `dev-workflow.md` (captured status before a branch, `set -euo pipefail`), test names `<function_under_test>_<scenario>`, explicit types on `let`, no assertion messages, comments only for a non-obvious reason. Fix what you find in its own commit before Step 6.

- [ ] **Step 6: Record the results and the deviations**

Append to [Verification notes](#verification-notes), under `### Automated suites`, one bullet per command of Steps 1 to 4 with its result. Then append a `## Deviations from the plan` section after the verification notes: new files or functions, steps which changed, outputs which differed, ledger claims which did not hold, checks not run and why. Write "None" under it if there are none.

- [ ] **Step 7: Commit**

```sh
git add docs/plans/2026-10-12-phase-9-verification.md
git status --short
git commit -m "phase 9 deviations"
```

Expected `git status --short` before the commit:

```text
M  docs/plans/2026-10-12-phase-9-verification.md
```

## Bug-fix ledger walk-through

Each bullet is a ledger entry (or a group of entries with one resolution) and the evidence for its resolution: tests (checked by Task 6) or a structural fact. "New" marks the regression tests of Tasks 3 to 5.

### Moot: the Node server is replaced

- Undefined `host`, missing Games functions, `kill()` and `spawn()` misuse: structural (typed Rust handlers returning `Result`); membership by `MemberId`: `leave_game_answers_left_game_and_the_others_see_the_member_leave`, `step_left_removes_the_member`, `admit_respawn_converts_a_spectator`, `spawn_member_converts_a_spectator_and_spawns_one_cell`.
- `removeMember` and the departing player's team: `finish_connection_removes_the_member_of_a_closed_connection`, `handle_request_connection_closed_queues_left_for_its_member`.
- `Games.remove()` leaks, Back in the creator's join menu: `run_game_task_removes_an_empty_game_after_the_grace_period`, `launch_game_registers_the_game_until_it_ends_empty`, `get_back_screen_leaves_lobby_screens_towards_the_title`.
- Round and force-spawn timeouts: structural (tick deadlines in `GameState`, no timers); a panicking game task: `supervise_game_unregisters_a_game_whose_task_panicked`.
- Stale broadcast index, `createGame` overwriting: `broadcast_queues_each_frame_for_every_recipient`, `reserve_game_allocates_increasing_game_ids`, `reserve_game_rejects_a_taken_title_and_keeps_no_slot`.
- `Object.fromEntries`: moot (no Node).
- Malformed payloads: `run_connection_closes_only_the_connection_which_sent_garbage`, `decode_and_try_from_never_panic_on_malformed_frames`, `dispatch_rejects_requests_outside_their_phase`.
- Leaving and spectating: `spectate_game_adds_a_spectator_which_follows_the_game`, `leave_game_then_join_another_game_receives_no_frame_of_the_first_game_after_left_game`.

### Wire shape mismatches

- Structural (one `MessageSerialIn` and `MessageSerialOut` for both ends); `decode_message_in_restores_every_variant`, `decode_message_out_restores_every_variant`.

### Client authority and trust

- Overwrites of board, teams, organisms, leaderboard; the spectator copy; the end-game title: structural (`MessageSerialIn` carries intents only; Task 6 Step 3); rows derived from state: `get_leaderboard_rows_orders_free_for_all_by_kills_then_deaths_then_id`.
- Damage on the victim's client, clients disagreeing: `two_client_sessions_compute_identical_checksums_for_one_game`.
- Player cap and stale join validation: `admit_join_rejects_at_the_member_limit`, `join_game_rejects_a_full_game`, `join_game_admits_only_one_of_two_joins_for_the_last_slot`, `admit_join_rejects_a_name_taken_by_a_member`, `admit_respawn_rejects_an_alive_or_unknown_member`.

### Passwords

- Enforced on the server, hashed: `join_game_checks_the_password`, `verify_password_matches_only_the_hashed_password`; sent once on submit: `build_join_request_joins_a_listed_game_with_the_entered_password`.
- Replacing a password, the security entry's lifetime, two callbacks: structural (the hash is set only by `CreateGame`, lives in `GameHandle`, one reply per request).

### Rooms, lobby, bandwidth

- Titles as rooms: `reserve_game_rejects_a_taken_title_and_keeps_no_slot`, `remove_game_keeps_a_newer_reservation_of_the_title`, `leave_game_then_join_another_game_receives_no_frame_of_the_first_game_after_left_game`.
- One lobby broadcast: `take_due_list_gives_a_changed_list_at_most_once_per_interval`, `subscribe_sends_the_current_list_at_once`.
- Full-state broadcasts: `encode_message_out_fits_pressing_input_bundles_within_size_budgets`, `encode_message_out_fits_a_full_game_snapshot_with_spores_in_flight_within_size_budget`.
- Game browser with a game listed: `receive_converts_a_game_list`, and Task 9; join menu team list: `get_summary_team_sizes_pairs_the_counts_with_the_teams_in_team_order`.

### Timing and frame-rate dependence

- Movement and projectile speed: `advance_moves_at_the_speed_along_one_axis`, `advance_projectile_covers_speed_times_flight_ticks`, `speeds_scale_the_original_packet_speeds_to_ticks`, and Task 9's speed measurement.
- Ability timers across respawn and title: structural (`OrganismAbilities` lives in the `Organism` each spawn creates anew; Task 6 Step 3).
- Countdown: `get_countdown_seconds_counts_down_the_delay`.
- Listeners on reconnect: structural (one socket, opened at start and by Reconnect; Task 6 Step 3).
- Cooldowns re-enabled from the HUD: `step_expires_a_cooldown_before_the_press_of_the_same_tick`, `ability_phase_expire_cools_then_readies_on_the_deadline`.

### Rounds

- Stuck progression: `step_runs_round_transitions_after_death_bookkeeping` and the round transition tests below.
- Pre-round counts: `run_round_transition_phase_counts_spectators_out`, `run_round_transition_phase_cancels_the_pre_round_below_the_minimum`.
- The win applied more than once: `run_round_transition_phase_gives_the_win_once_while_the_survivor_lives_on` (new).
- Force-spawned spectators: `force_spawn_participants_never_spawns_a_spectator`.
- World restored: `run_round_transition_phase_restores_the_world_and_force_spawns_after_the_post_round`, `restore_initial_bounds_undoes_shrinking`.
- Survivor detection: `run_round_transition_phase_gives_the_sole_survivor_a_win`.

### Simulation rule bugs fixed

- NaN aim: `launch_shot_ignores_a_zero_aim`; wraparound: `select_shot_cell_handles_aims_across_the_negative_x_axis`.
- Shot-secretion neutralize check: `run_damage_phase_spares_cells_inside_any_neutralize_field_from_a_shot_secretion` (new), `run_damage_phase_spares_cells_inside_any_neutralize_field`, `press_shot_slot_spares_cells_inside_the_targets_own_neutralize_field`.
- Spawn overlap past the first organism: `is_spawn_position_valid_checks_every_organism_after_the_first` (new), `find_spawn_position_gives_up_after_the_attempt_limit`; rectangle retry: `find_spawn_position_stays_inside_the_margin`.
- Border rule: `contains_cell_ellipse_is_centered_on_offset_bounds`, `contains_cell_rectangle_follows_an_offset_origin`, `run_birth_phase_skips_sites_outside_the_world_without_a_draw`, `run_natural_death_phase_removes_cells_outside_the_world_without_a_draw`.
- Quadratic natural death: structural (dense bitmap, one pass per phase); `retighten_shrinks_the_box_to_the_remaining_cells`.
- `removeCell` fallthrough: `remove_clears_only_that_cell`.
- Friendly-fire guard: `press_shot_slot_spares_teammates`, `run_damage_phase_spares_teammates_of_the_owner`.
- Own effects a round trip late: structural (presses precede growth in `step`; Task 6 Step 3).
- Two clients disagreeing: `join_game_sends_a_snapshot_then_bundles_and_every_checksum_matches`, `two_client_sessions_compute_identical_checksums_for_one_game`.

### Kept as deferred rules

- Rule 1, own acid: `run_damage_phase_lets_acid_reach_its_owner`.
- Rule 2, stale kill credit: `step_records_a_death_with_kill_credit`, `step_gives_no_credit_to_a_departed_hitter`; teammate and neutralized hits record nothing: `run_damage_phase_spares_teammates_of_the_owner`, `run_damage_phase_records_no_hitter_for_neutralized_acid`.
- Rule 3: `run_birth_phase_lets_teammates_block_each_other`; rule 4: `run_birth_phase_rolls_a_shared_site_once_per_neighboring_cell`, `adjacent_sites_repeat_a_site_once_per_neighboring_cell`.
- Rule 5: `run_damage_phase_spares_cells_inside_any_neutralize_field`; rule 6: `press_third_centres_the_field_on_the_cursor`; rule 7: `launch_shot_can_take_the_last_cell`.
- Spore at the centroid: `launch_spores_drops_a_spore_at_the_centroid`.

### UI and menus

- Every player's neutralize drawn: `tick_layer_from_state_draws_toxin_fields_then_secretions_then_neutralize_fields`.
- Spectator respawning into a team game: `admit_respawn_converts_a_spectator`, `get_respawn_values_give_a_spectator_the_join_defaults`.
- Screen-name uniqueness: `admit_join_rejects_a_name_taken_by_a_member`, `join_game_rejects_a_taken_screen_name`.
- Cap against minimum: `validate_applies_the_cross_field_rules_of_the_mode_only`, `validate_counts_a_missing_required_field_as_below_range`.
- Smallest-team preselection: `join_defaults_preselect_the_first_smallest_team`.
- Leaderboard spectator skip: `get_leaderboard_rows_leaves_out_pure_spectators`, `get_leaderboard_rows_cuts_player_rows_to_the_leaderboard_length`; sort by the board's mode: `get_leaderboard_rows_orders_survival_by_kills_then_wins_then_id`, `from_state_lists_skirmish_teams_and_emphasises_the_own_team`.
- Game-closed paths to an animated title: `after_connection_lost_returns_to_the_title_unless_in_the_tutorial`, and Task 9.
- Tutorial bot arguments, per-organism respawn, timers on leave: `create_tutorial_bot_joiner_places_the_bot_inside_the_world_away_from_the_player`, `advance_frame_places_every_organism_again_at_its_own_cursor_after_it_dies`; timers: structural (tick deadlines owned by `Tutorial`, dropped with it).
- Respawn team balance: `admit_respawn_lets_a_member_stay_on_its_team_and_excludes_it_from_the_count`.
- Name-label truncation and centring: `get_shown_name_cuts_names_over_thirty_characters_to_twenty_and_an_ellipsis`, `get_name_labels_places_a_label_under_the_centroid_in_view_pixels`.
- `Board.find`, field coupling, error keys: `find_game_summary_finds_the_listed_game_by_id`, `resolve_game_settings_includes_only_the_fields_of_the_mode`, `place_issues_moves_issues_of_hidden_or_no_fields_to_the_general_row`; `Message.width`: structural (the DOM measures the box).
- `getMpos`: `get_aim_vector_is_the_rounded_offset_from_the_crosshair`; `alert()`: structural (none in `web/src`; Task 6 Step 3); unimplemented modes: moot.

## Coverage of the roadmap

- wasm replay test under wasmtime (`wasm32-wasip1`): Task 2, with Task 1's shared program check.
- Growth statistics script: Task 7 (the script and test exist since phase 2).
- Manual verification list: Task 9 (visual parity, Chrome backends and pixel ratios, feel, two browsers and throttling; Safari left to the owner).
- Bug-fix ledger walk-through: Tasks 3 to 5 (regression tests) and Task 6 (the mapping above, checked).
- Documentation pass: Task 8.
- Done condition: every automated suite in Task 10; the manual checks recorded in the verification notes for the PR.

## PR description

**Problem:** the rewrite's determinism had not been checked on a 32-bit wasm target outside the browser, the manual verification list and the bug-fix ledger had not been walked, and the architecture's module trees had drifted from the code.

**Solution:** `scripts/test/test-replay-wasm.sh` runs the replay fixture test built for `wasm32-wasip1` under wasmtime against the committed checksums; three regression tests pin ledger fixes which had no test of their own (spawn search past the first organism, one win per round, neutralize against shot secretions); the module trees, script list and test list in `docs/architecture/` match the code again. The verification notes record the growth statistics, the ledger walk-through and the manual browser checks. Repository: `bacter-rs` only.

## Verification notes

### Automated suites

### Growth statistics

- `./scripts/test/test-growth-statistics.sh`: PASS, every state's means within 10% (cells) and 15% (changes per tick) of the reference table in `testing.md`:

```text
Default Still: cells 64.0 (reference 64.0), changes per tick 6.22 (reference 6.2)
Default Moving: cells 42.6 (reference 42.5), changes per tick 7.78 (reference 7.7)
Compress Still: cells 37.7 (reference 37.8), changes per tick 0.79 (reference 0.8)
Compress Moving: cells 23.5 (reference 23.0), changes per tick 4.46 (reference 4.4)
Extend Still: cells 124.1 (reference 124.8), changes per tick 7.97 (reference 7.8)
Extend Moving: cells 82.3 (reference 81.2), changes per tick 10.42 (reference 10.4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.68s
```

### Bug-fix ledger walk-through

- Mapping check (`/tmp/bacter-phase9-check-ledger-tests.sh`): `98 tests named, 0 problems`.
- No `alert(` in the web client: `grep -rn 'alert(' web/src` printed nothing.
- Client messages carry intents only: `MessageSerialIn` has `SubscribeGameList`, `UnsubscribeGameList`, `CreateGame`, `JoinGame`, `SpectateGame`, `Respawn`, `UpdateAppearance`, `Input`, `RequestSnapshot`, `LeaveGame`; none carries a board, a leaderboard, a member list, a password change or an end-game request.
- Tick order: `step` runs `activation::run_ability_press_phase` before `projectile::run_flight_phase`, `growth::run_birth_phase`, `growth::run_natural_death_phase` and `damage::run_damage_phase`.
- Fresh abilities on spawn: both spawn paths assign `Some(Organism::new(position))` (`shared/src/organism/spawn.rs:85`, `shared/src/organism/spawn.rs:139`); `Organism::new` sets `abilities: OrganismAbilities::all_ready()` (`shared/src/organism/organism.rs:50`).
- One socket, no automatic reconnect: `connect()` is called from `start` once the driver is created and from `reconnect`, which returns while `self.socket` is set and runs only for `DriverCommandKind::Reconnect` (dispatched only by the title menu's reconnect button, shown while the connection status offers it); `GameSocket::open` is called only from `open_socket`, which only `connect` reaches.
- Other structural claims read against the code, all held: the password hash is computed only in `CreateGame` handling (`server/src/socket/request_dispatch.rs:165`) and stored in `GameHandle::password_hash`; round and tutorial timing are tick values (`phase_started_at: Tick` in the round state, `task_deadline: Option<Tick>` in `Tutorial`), so no game rule depends on a wall-clock timer; cell occupancy is a row bitmap (`CellOccupancy::row_bitmap_bytes`); `MessageBox` is a DOM `<p>` whose width the browser lays out; `MessageSerialIn` and `MessageSerialOut` are defined once in `shared/src/protocol/` and used by both `server` and `web`.

### Manual checks
