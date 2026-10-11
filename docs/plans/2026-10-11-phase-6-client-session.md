# Phase 6: client session implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** the platform-independent client in `shared`: a sans-IO `ClientSession` which follows a server game from snapshots, bundles and checksums, resyncs, and samples the player's controls into inputs; the frame pacer, crosshair controller, keyboard edges, own member state, message texts and projectile interpolation every scene uses; the title scene and the tutorial as local games; the settings store contract; proven by unit tests of every module and by a headless test in which two `ClientSession`s play one game against the in-process server and compute identical checksums.

**Architecture:** `docs/architecture/client-web.md` (client session, frame pacing and driving; tutorial and title; settings). `ClientSession` decodes each server frame, converts it with `TryFrom`, steps bundles with the shared `step`, and returns `ClientEffect`s, among them `SendFrame` for every frame to write; the platform owns the socket and the clock. `PlayerControls` (keyboard edges plus the crosshair with its sent-cursor history) is owned by the platform and lent to every scene. The title scene and the tutorial run `LocalGame`s: local `GameState`s stepped with local bundles from the `FramePacer` accumulator. `ActiveScene` dispatches `advance_frame` to whichever scene is active.

**Tech Stack:** Rust 1.95 (edition 2024), the existing `shared` dependencies (bitcode inside `protocol/` only, libm, minimer, log); server dev dependency tokio-tungstenite 0.26 for the headless test; no new dependencies.

---

## Ground rules for the executor

- Repository `/Users/singularity/bacter-rs`, branch `client-session`. Run `git -C /Users/singularity/bacter-rs branch --show-current` first; stop if it does not print `client-session`.
- Every command runs from the repository root `/Users/singularity/bacter-rs`.
- Never switch branches, push, rebase or amend. Stage by explicit path, run `git status` before each commit, one-line commit messages without attribution.
- Never run `cargo clippy`.
- The code below is `rustfmt`-formatted with the repository's `rustfmt.toml`; transcribe it exactly, so `cargo fmt --all -- --check` stays silent.
- Edits take these forms:
  - "Create `path`" with the whole new file;
  - "Replace the whole contents of `path`";
  - "In `path`, replace" a text which occurs exactly once in the file, "with" its replacement;
  - "replace everything above `#[cfg(test)]`" (the file's one test module) with the given code, followed by one blank line;
  - "append inside `mod tests`, directly above its closing brace", which is the last line of the file, after one blank line separating it from the item before;
  - "append to the end of `path`, after one blank line".
- A new file's failing test is the file holding only its `#[cfg(test)] mod tests` block (together with its `pub mod` line); the implementation step then replaces everything above `#[cfg(test)]`, which is nothing yet, with the code.
- `mod.rs` files and `shared/src/lib.rs` are given whole each time they change.
- Builds stay warning-free, tests included; a warning is a defect to fix before committing.
- Expected outputs were observed while writing this plan, by executing it end to end in a scratch copy of this branch; test counts and filtered counts assume the tasks are done in order with the code exactly as given. The `finished in` timing of each `test result` line is left out. A compile failure is quoted by its first `error` line; later lines differ only in the names they report.
- Task 12 starts an in-process server on `127.0.0.1:0`; the sandbox must allow binding loopback ports. A bind failing with `Operation not permitted (os error 1)` (`the test server binds` panics) means the command ran sandboxed: run it from a script in `/tmp`, in the foreground.
- `shared/tests/forbidden_operations.rs` scans `shared/src` outside `render/` and `play/`, test modules included: `tutorial/`, `title/` and `settings/` use no std float functions, `HashMap` or `HashSet` (the settings test store is a `BTreeMap`).

## Decisions taken where the design is silent or contradictory

- Session ownership: `ActiveScene::Online(ClientSession)` as designed. The connection's session must also receive the game list while the title scene animates behind the title screen, the browser and the menus, so outside a game the platform keeps the session beside the `Title` scene and moves it into `Online` for a game; that wiring is the phase 8 driver's.
- `ClientEffect::ConnectionLost(Option<CloseReasonKind>)`: `None` for an abnormal close (1006), a failed open or a close code no close reason uses, whose notice is `UNREACHABLE_CLIENT_NOTICE`. `docs/architecture/client-web.md` is updated (Task 19).
- `ClientSession` also has `advance_frame` (the `Online` arm of `ActiveScene::advance_frame`: crosshair movement, the snapshot re-request, the applied tick and the interpolation alpha), `game_id`, `own_member_id` and `message_text` (the game message, or "Catching up" while a resync waits for its snapshot).
- Synchronisation:
  - bundles before the first snapshot, and while a requested snapshot is awaited, are discarded; a snapshot for another game id is dropped;
  - a resync trigger while a resync is in progress starts nothing new; the request is re-sent after 5 s without a snapshot, and re-sends do not count against the limit;
  - the sixth resync start within 60 s loses synchronisation: the session sends `LeaveGame`, reports `SynchronisationLost`, and swallows the `LeftGame` which follows;
  - the backlog is checked after every received frame, against the snapshot tick plus the whole tick periods since the snapshot arrived;
  - a checksum is compared only when the applied state is at its tick.
- Crosshair:
  - position kept in unrounded subpixels and clamped to the protocol's ±2^28 subpixel cursor limit, since the server closes a connection whose cursor exceeds it;
  - the first snapshot of a membership places it at the world centre (a spectator's starting camera; a joiner's spawn snaps it again);
  - a correction shifts the history entry of the corrected tick and every later one, so a repeated correction for one tick is zero; a snap (own spawn, correction past the history) clears the history;
  - it does not move before the own member's `Joined` is applied (no own member state, speed 0).
- Keyboard: the aim of a shot-slot press is the latest pointer aim the platform reported (`KeyboardState::aim_changed`); `release_all_keys` is for a lost window focus, after which no keyup arrives.
- Transient messages: `TransientMessageKind` (its texts) and `TRANSIENT_MESSAGE_DURATION_MILLISECONDS` live in `play/message_text.rs`; showing them for 3 s is the platform's, on `OwnSpawnRejected` and on the R key outcome `own_member_state::get_respawn_key_outcome` gives.
- Local games: the new `play/local_game.rs` holds what the title scene and the tutorial share (local settings from the window, joining in tick 1, stepping, placing missing organisms, resize, the scene RNG, colour and coordinate draws). The scene's own draws come from `Pcg32::from_seed_and_stream(seed, LOCAL_SCENE_RNG_STREAM)`, so they never consume the game RNG which `GameState::new(settings, seed)` seeds from the same platform seed.
- Placement on integer pixels: a dummy's or the bot's cursor is drawn one pixel further in than the original's minimum, so its first cell is inside the world under the inclusive border rule; the bot is drawn inside the world (the original drew from the window origin) and takes the last of at most 64 candidates in a world too small for the 80 px distance.
- Title: a dummy which reaches zero cells is placed again at its cursor, as tutorial organisms are.
- Tutorial:
  - resize is supported like the title's (the fullscreen task changes the window size); the crosshair and the bot's cursor are rescaled;
  - the platform reports the display mode (`DisplayModeKind`, `display_mode_changed`), which the move task reads to skip the fullscreen task;
  - task conditions become tick deadlines: an ability task ends when the ability it pressed stops being active, the compress and freeze tasks when the effect on the bot ends, the toxin task one toxin duration after the bot is hit, the spore task one secretion duration after a secretion hits the bot;
  - "hit" means the bot's `last_hitter` is the player, or the bot died with the kill credited to the player;
  - the spore pause stops stepping and the crosshair until a fourth-ability press is pending;
  - with `InputFocusKind::Menu` (the pause tutorial menu) the tutorial keeps stepping, as the original's intervals kept running, but the crosshair and the held-key tasks wait.
- Settings: `ClientSettingKind { NameLabels, Messages }`, its storage keys and the `NAME_LABELS_SETTING` and `MESSAGES_SETTING` constants live in `shared/src/settings/` beside eafora's framework, so every platform reads the same keys and defaults.
- Shared additions: `SubpixelPoint::to_rounded_world_point` moves from the server's bundle builder into `geometry`, since the tutorial rounds its cursor as the server does (Task 1); `AbilityPressSet::intersection` masks the tutorial's presses (Task 2); `ORGANISM_COLOR_ORDER` and `SKIN_ORDER` list the palette (Task 13).
- Interpolation: a flying projectile is drawn at `position - velocity * (1 - alpha)`, since it moved by exactly its velocity in the last applied tick; `InterpolationFrame { alpha }` is what phase 7's `RenderScene::from_state` takes.
- The headless test is `server/tests/client_session.rs`, since it needs the in-process server; `TestClient::receive_binary_frame` joins the shared helper.

## File structure

- Modify `shared/src/geometry/geometry.rs`: `SubpixelPoint::to_rounded_world_point`.
- Modify `server/src/game/bundle_builder.rs`: uses it in place of its own rounding.
- Modify `shared/src/ability/ability_model.rs`: `AbilityPressSet::intersection`.
- Modify `shared/src/member/member.rs`: `ORGANISM_COLOR_ORDER`, `SKIN_ORDER`.
- Modify `shared/src/lib.rs`: declares `play`, `settings`, `title`, `tutorial`.
- Create `shared/src/settings/mod.rs`, `shared/src/settings/settings.rs`: `SettingKey`, `SettingValue`, `SettingsStore`, `Setting`, `ClientSettingKind` and the two settings.
- Create `shared/src/play/mod.rs`: declares the play modules.
- Create `shared/src/play/game_key.rs`: `GameKeyKind`, `KeyTransitionKind`, `KeyRepeatKind`, `AbilityPressSample`, `MovementDirection`, `KeyboardState`.
- Create `shared/src/play/crosshair.rs`: speeds, `SentCursor`, `SentCursorHistory`, `CrosshairController`.
- Create `shared/src/play/player_controls.rs`: `PlayerControls`.
- Create `shared/src/play/frame_pacing.rs`: `FramePacer`, `FrameOutput`, the frame and tick limits.
- Create `shared/src/play/interpolation.rs`: `InterpolationFrame`, `InterpolatedPoint`.
- Create `shared/src/play/own_member_state.rs`: `OwnMemberStateKind`, `RespawnKeyOutcomeKind`, `get_own_member_state`, `get_respawn_key_outcome`.
- Create `shared/src/play/message_text.rs`: `get_message_text`, `TransientMessageKind`, `CATCHING_UP_TEXT`.
- Create `shared/src/play/client_session.rs`: `ClientSession`, `ClientEffect`, synchronisation limits.
- Create `shared/src/play/active_scene.rs`: `InputFocusKind`, `ActiveScene`.
- Create `shared/src/play/local_game.rs`: `LocalGame`, `LocalJoiner`, `PixelSize`, local settings, scene RNG and draws.
- Create `shared/src/title/mod.rs`, `shared/src/title/title_scene.rs`: `TitleScene`.
- Create `shared/src/tutorial/mod.rs`, `shared/src/tutorial/tutorial_bot.rs`: `TutorialBot`, bot placement.
- Create `shared/src/tutorial/tutorial.rs`: `TutorialTaskKind`, `DisplayModeKind`, `TutorialSteppingKind`, `Tutorial`.
- Modify `server/tests/helpers/test_server.rs`: `TestClient::receive_binary_frame`.
- Create `server/tests/client_session.rs`: two `ClientSession`s against the in-process server.
- Modify `docs/architecture/overview.md`, `docs/architecture/client-web.md`, `docs/architecture/testing.md`: the module tree, `ConnectionLost`, the further `ClientSession` functions, the headless test.

---

### Task 1: Subpixel rounding in shared geometry

**Files:**
- Modify: `shared/src/geometry/geometry.rs`
- Modify: `server/src/game/bundle_builder.rs`
- Test: inline `mod tests` in `shared/src/geometry/geometry.rs`

The tutorial sends its crosshair to its local game rounded to whole pixels, as the server rounds the clamped cursor of every bundle (protocol.md, input bundle). The rounding moves from the server's bundle builder into `geometry`, with its test, and the bundle builder calls it.

- [ ] **Step 1: Write the failing test**

In `shared/src/geometry/geometry.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn to_rounded_world_point_rounds_halves_up_on_both_signs() {
        assert_eq!(
            SubpixelPoint { x: 1535, y: -1536 }.to_rounded_world_point(),
            WorldPoint { x: 1, y: -1 },
        );
        assert_eq!(
            SubpixelPoint { x: 1536, y: -1537 }.to_rounded_world_point(),
            WorldPoint { x: 2, y: -2 },
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib geometry::geometry`

Expected: compile failure, first line ``error[E0599]: no method named `to_rounded_world_point` found for struct `geometry::geometry::SubpixelPoint` in the current scope``.

- [ ] **Step 3: Implement**

In `shared/src/geometry/geometry.rs`, replace:

```rust
    pub fn distance_squared(self, other: SubpixelPoint) -> i64 {
        widened_distance_squared(self.x, self.y, other.x, other.y)
    }
}
```

with:

```rust
    pub fn distance_squared(self, other: SubpixelPoint) -> i64 {
        widened_distance_squared(self.x, self.y, other.x, other.y)
    }

    /// Halves round up.
    pub fn to_rounded_world_point(self) -> WorldPoint {
        WorldPoint {
            x: get_rounded_pixels(self.x),
            y: get_rounded_pixels(self.y),
        }
    }
}
```

In `shared/src/geometry/geometry.rs`, replace:

```rust
fn widened_distance_squared(first_x
```

with:

```rust
fn get_rounded_pixels(subpixels: i32) -> i32 {
    (subpixels + SUBPIXELS_PER_PIXEL / 2).div_euclid(SUBPIXELS_PER_PIXEL)
}

fn widened_distance_squared(first_x
```

In `server/src/game/bundle_builder.rs`, replace:

```rust
use shared::geometry;
use shared::geometry::{SubpixelPoint, WorldPoint};
```

with:

```rust
use shared::geometry::{SubpixelPoint, WorldPoint};
```

In `server/src/game/bundle_builder.rs`, replace:

```rust
let cursor: WorldPoint = get_rounded_world_point(applied_cursor);
```

with:

```rust
let cursor: WorldPoint = applied_cursor.to_rounded_world_point();
```

In `server/src/game/bundle_builder.rs`, delete these two functions and the blank line after them, directly above `#[cfg(test)]`:

```rust
/// Halves round up.
fn get_rounded_world_point(subpixel_point: SubpixelPoint) -> WorldPoint {
    WorldPoint {
        x: get_rounded_pixels(subpixel_point.x),
        y: get_rounded_pixels(subpixel_point.y),
    }
}

fn get_rounded_pixels(subpixels: i32) -> i32 {
    (subpixels + geometry::SUBPIXELS_PER_PIXEL / 2).div_euclid(geometry::SUBPIXELS_PER_PIXEL)
}
```

In `server/src/game/bundle_builder.rs`, delete this test, the last one in `mod tests`, with the blank line above it:

```rust
    #[test]
    fn get_rounded_world_point_rounds_halves_up_on_both_signs() {
        assert_eq!(
            get_rounded_world_point(SubpixelPoint { x: 1535, y: -1536 }),
            WorldPoint { x: 1, y: -1 },
        );
        assert_eq!(
            get_rounded_world_point(SubpixelPoint { x: 1536, y: -1537 }),
            WorldPoint { x: 2, y: -2 },
        );
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared --lib geometry::geometry`

Expected: `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 328 filtered out`.

Run: `cargo test -p server --lib game::bundle_builder`

Expected: `test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 109 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/geometry/geometry.rs server/src/game/bundle_builder.rs
git commit -m "subpixel rounding in shared geometry"
```

### Task 2: Ability press set intersection

**Files:**
- Modify: `shared/src/ability/ability_model.rs`
- Test: inline `mod tests` in `shared/src/ability/ability_model.rs`

The tutorial enables one ability at a time by masking the sampled presses with the task's enabled set before each local bundle (client-web.md, tutorial and title).

- [ ] **Step 1: Write the failing test**

In `shared/src/ability/ability_model.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn intersection_keeps_only_the_presses_in_both_sets() {
        let pressed: AbilityPressSet = AbilityPressSet::FIRST.with(AbilityPressSet::THIRD);
        let enabled: AbilityPressSet = AbilityPressSet::THIRD.with(AbilityPressSet::FOURTH);

        assert_eq!(pressed.intersection(enabled), AbilityPressSet::THIRD);
        assert_eq!(pressed.intersection(AbilityPressSet::NONE), AbilityPressSet::NONE);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib ability::ability_model`

Expected: compile failure, first line ``error[E0599]: no method named `intersection` found for struct `ability_model::AbilityPressSet` in the current scope``.

- [ ] **Step 3: Implement**

In `shared/src/ability/ability_model.rs`, replace:

```rust
    pub fn with(self, press: AbilityPressSet) -> AbilityPressSet {
        AbilityPressSet {
            bits: self.bits | press.bits,
        }
    }
```

with:

```rust
    pub fn with(self, press: AbilityPressSet) -> AbilityPressSet {
        AbilityPressSet {
            bits: self.bits | press.bits,
        }
    }

    pub fn intersection(self, other: AbilityPressSet) -> AbilityPressSet {
        AbilityPressSet {
            bits: self.bits & other.bits,
        }
    }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib ability::ability_model`

Expected: `test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 315 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/ability/ability_model.rs
git commit -m "ability press set intersection"
```

### Task 3: Client settings

**Files:**
- Create: `shared/src/settings/mod.rs`
- Create: `shared/src/settings/settings.rs`
- Modify: `shared/src/lib.rs`
- Test: inline `mod tests` in `shared/src/settings/settings.rs`

eafora's settings framework (`shared/src/settings.rs` there): a typed key and value, a store each platform implements, and a setting pairing its key with its default. The two settings of the pause menus, name labels and messages, both default to on (client-web.md, settings). The test store is a `BTreeMap`, since the forbidden-operations scan covers `settings/`.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/lib.rs`:

```rust
//! Everything the server and the clients must agree on, plus platform-independent client logic.

pub mod ability;
pub mod error;
pub mod game;
pub mod geometry;
pub mod member;
pub mod organism;
pub mod protocol;
pub mod random;
pub mod replay;
pub mod round;
pub mod settings;
pub mod world;
```

Create `shared/src/settings/mod.rs`:

```rust
pub mod settings;

pub use settings::*;
```

Create `shared/src/settings/settings.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct InMemorySettingsStore {
        entries: RefCell<BTreeMap<&'static str, Vec<u8>>>,
    }

    impl SettingsStore for InMemorySettingsStore {
        fn load<K: SettingKey>(&self, key: K) -> Option<SettingValue> {
            let bytes: Vec<u8> = self.entries.borrow().get(key.storage_key()).cloned()?;

            SettingValue::from_bytes(&bytes)
        }

        fn store<K: SettingKey>(&self, key: K, value: SettingValue) {
            self.entries.borrow_mut().insert(key.storage_key(), value.to_bytes());
        }
    }

    #[test]
    fn read_returns_the_default_when_absent() {
        let store: InMemorySettingsStore = InMemorySettingsStore::default();

        assert!(NAME_LABELS_SETTING.read(&store));
        assert!(MESSAGES_SETTING.read(&store));
    }

    #[test]
    fn write_then_read_round_trips_through_the_store() {
        let store: InMemorySettingsStore = InMemorySettingsStore::default();

        MESSAGES_SETTING.write(&store, false);
        assert!(!MESSAGES_SETTING.read(&store));
        assert!(NAME_LABELS_SETTING.read(&store));

        MESSAGES_SETTING.write(&store, true);
        assert!(MESSAGES_SETTING.read(&store));
    }

    #[test]
    fn storage_key_differs_per_setting() {
        assert_eq!(ClientSettingKind::NameLabels.storage_key(), "name_labels");
        assert_eq!(ClientSettingKind::Messages.storage_key(), "messages");
    }

    #[test]
    fn from_bytes_restores_every_value() {
        for value in [SettingValue::Bool(true), SettingValue::Bool(false)] {
            assert_eq!(SettingValue::from_bytes(&value.to_bytes()), Some(value));
        }
    }

    #[test]
    fn from_bytes_rejects_empty_truncated_and_unknown_tag_bytes() {
        assert_eq!(SettingValue::from_bytes(&[]), None);
        assert_eq!(SettingValue::from_bytes(&[BOOL_TAG]), None);
        assert_eq!(SettingValue::from_bytes(&[9, 9]), None);
    }

    #[test]
    fn read_falls_back_to_the_default_on_undecodable_bytes() {
        let store: InMemorySettingsStore = InMemorySettingsStore::default();
        store.entries.borrow_mut().insert("name_labels", vec![0xff, 0xff]);

        assert!(NAME_LABELS_SETTING.read(&store));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib settings::settings`

Expected: compile failure, first line ``error[E0405]: cannot find trait `SettingsStore` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/settings/settings.rs`, replace everything above `#[cfg(test)]` with:

```rust
const BOOL_TAG: u8 = 1;
const NAME_LABELS_STORAGE_KEY: &str = "name_labels";
const MESSAGES_STORAGE_KEY: &str = "messages";

pub const NAME_LABELS_SETTING: Setting<ClientSettingKind, bool> = Setting::new(ClientSettingKind::NameLabels, true);
pub const MESSAGES_SETTING: Setting<ClientSettingKind, bool> = Setting::new(ClientSettingKind::Messages, true);

pub trait SettingKey: Copy {
    /// Stable across releases; stored values are found by it.
    fn storage_key(self) -> &'static str;
}

/// A leading tag byte makes the bytes self-describing, so a corrupt or wrong-typed value fails to decode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingValue {
    Bool(bool),
}

impl SettingValue {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            SettingValue::Bool(value) => vec![BOOL_TAG, u8::from(*value)],
        }
    }

    /// `None` for empty or truncated bytes or an unknown tag.
    pub fn from_bytes(bytes: &[u8]) -> Option<SettingValue> {
        match bytes {
            [BOOL_TAG, 0] => Some(SettingValue::Bool(false)),
            [BOOL_TAG, 1] => Some(SettingValue::Bool(true)),
            _ => None,
        }
    }
}

impl From<bool> for SettingValue {
    fn from(value: bool) -> SettingValue {
        SettingValue::Bool(value)
    }
}

impl TryFrom<SettingValue> for bool {
    type Error = SettingValue;

    fn try_from(value: SettingValue) -> Result<bool, SettingValue> {
        match value {
            SettingValue::Bool(inner) => Ok(inner),
        }
    }
}

/// A persistent store for user settings, addressed by a typed key and value.
pub trait SettingsStore {
    fn load<K: SettingKey>(&self, key: K) -> Option<SettingValue>;
    fn store<K: SettingKey>(&self, key: K, value: SettingValue);
}

/// A key with its default.
pub struct Setting<K, T> {
    key: K,
    default: T,
}

impl<K, T> Setting<K, T>
where
    K: SettingKey,
    T: Clone + Into<SettingValue> + TryFrom<SettingValue>,
{
    pub const fn new(key: K, default: T) -> Setting<K, T> {
        Setting { key, default }
    }

    /// The default when the store has no value or an undecodable one.
    pub fn read(&self, store: &impl SettingsStore) -> T {
        store
            .load(self.key)
            .and_then(|value| T::try_from(value).ok())
            .unwrap_or_else(|| self.default.clone())
    }

    pub fn write(&self, store: &impl SettingsStore, value: T) {
        store.store(self.key, value.into());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientSettingKind {
    NameLabels,
    Messages,
}

impl SettingKey for ClientSettingKind {
    fn storage_key(self) -> &'static str {
        match self {
            ClientSettingKind::NameLabels => NAME_LABELS_STORAGE_KEY,
            ClientSettingKind::Messages => MESSAGES_STORAGE_KEY,
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib settings::settings`

Expected: `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 337 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/lib.rs shared/src/settings/mod.rs shared/src/settings/settings.rs
git commit -m "client settings"
```

### Task 4: Game keys and press edges

**Files:**
- Create: `shared/src/play/mod.rs`
- Create: `shared/src/play/game_key.rs`
- Modify: `shared/src/lib.rs`
- Test: inline `mod tests` in `shared/src/play/game_key.rs`

Platforms map native key events to `GameKeyKind` and pass `KeyboardState::key_changed`. Held keys steer the crosshair; ability keys record keydown edges, a repeated keydown ignored, accumulated until the next sample takes or discards them; a shot-slot press carries the pointer aim (protocol.md, client to server, input sampling).

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/lib.rs`:

```rust
//! Everything the server and the clients must agree on, plus platform-independent client logic.

pub mod ability;
pub mod error;
pub mod game;
pub mod geometry;
pub mod member;
pub mod organism;
pub mod play;
pub mod protocol;
pub mod random;
pub mod replay;
pub mod round;
pub mod settings;
pub mod world;
```

Create `shared/src/play/mod.rs`:

```rust
pub mod game_key;

pub use game_key::*;
```

Create `shared/src/play/game_key.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const AIM: AimVector = AimVector { x: 30, y: -40 };

    fn press(keyboard: &mut KeyboardState, key: GameKeyKind) {
        keyboard.key_changed(key, KeyTransitionKind::Pressed, KeyRepeatKind::Initial);
    }

    #[test]
    fn key_changed_records_a_keydown_edge_of_an_ability_key() {
        let mut keyboard: KeyboardState = KeyboardState::new();

        press(&mut keyboard, GameKeyKind::ThirdAbility);

        assert_eq!(
            keyboard.take_ability_presses(),
            AbilityPressSample {
                ability_presses: AbilityPressSet::THIRD,
                aim: None,
            },
        );
    }

    #[test]
    fn key_changed_ignores_a_repeated_keydown() {
        let mut keyboard: KeyboardState = KeyboardState::new();

        keyboard.key_changed(
            GameKeyKind::FourthAbility,
            KeyTransitionKind::Pressed,
            KeyRepeatKind::Repeated,
        );

        assert_eq!(keyboard.take_ability_presses(), AbilityPressSample::NONE);
        assert!(keyboard.is_held(GameKeyKind::FourthAbility));
    }

    #[test]
    fn take_ability_presses_returns_the_edges_since_the_last_sample_and_clears_them() {
        let mut keyboard: KeyboardState = KeyboardState::new();
        press(&mut keyboard, GameKeyKind::FirstAbility);
        keyboard.key_changed(
            GameKeyKind::FirstAbility,
            KeyTransitionKind::Released,
            KeyRepeatKind::Initial,
        );
        press(&mut keyboard, GameKeyKind::ThirdAbility);

        assert_eq!(
            keyboard.take_ability_presses().ability_presses,
            AbilityPressSet::FIRST.with(AbilityPressSet::THIRD),
        );
        assert_eq!(keyboard.take_ability_presses(), AbilityPressSample::NONE);
    }

    #[test]
    fn key_changed_takes_the_pointer_aim_with_a_shot_slot_press() {
        let mut keyboard: KeyboardState = KeyboardState::new();
        keyboard.aim_changed(AIM);

        press(&mut keyboard, GameKeyKind::SecondAbility);

        assert_eq!(
            keyboard.take_ability_presses(),
            AbilityPressSample {
                ability_presses: AbilityPressSet::SECOND,
                aim: Some(AIM),
            },
        );
    }

    #[test]
    fn discard_ability_presses_drops_the_edges_made_without_an_organism() {
        let mut keyboard: KeyboardState = KeyboardState::new();
        keyboard.aim_changed(AIM);
        press(&mut keyboard, GameKeyKind::FirstAbility);

        keyboard.discard_ability_presses();

        assert_eq!(keyboard.pending_ability_presses(), AbilityPressSet::NONE);
        assert_eq!(keyboard.take_ability_presses(), AbilityPressSample::NONE);
    }

    #[test]
    fn key_changed_presses_no_ability_for_other_keys() {
        let mut keyboard: KeyboardState = KeyboardState::new();

        for key in [
            GameKeyKind::MoveUp,
            GameKeyKind::Respawn,
            GameKeyKind::Pause,
            GameKeyKind::Fullscreen,
        ] {
            press(&mut keyboard, key);
        }

        assert_eq!(keyboard.take_ability_presses(), AbilityPressSample::NONE);
        assert!(keyboard.is_held(GameKeyKind::Fullscreen));
    }

    #[test]
    fn movement_direction_cancels_opposite_keys() {
        let mut keyboard: KeyboardState = KeyboardState::new();
        press(&mut keyboard, GameKeyKind::MoveLeft);
        press(&mut keyboard, GameKeyKind::MoveUp);

        assert_eq!(keyboard.movement_direction(), MovementDirection { x: -1, y: -1 });

        press(&mut keyboard, GameKeyKind::MoveRight);

        assert_eq!(keyboard.movement_direction(), MovementDirection { x: 0, y: -1 });
        assert!(keyboard.is_any_movement_key_held());
    }

    #[test]
    fn release_all_keys_stops_every_held_key() {
        let mut keyboard: KeyboardState = KeyboardState::new();
        press(&mut keyboard, GameKeyKind::MoveDown);

        keyboard.release_all_keys();

        assert_eq!(keyboard.movement_direction(), MovementDirection { x: 0, y: 0 });
        assert!(!keyboard.is_any_movement_key_held());
    }

    #[test]
    fn masked_keeps_the_enabled_presses_and_drops_an_aim_without_a_shot_slot_press() {
        let sample: AbilityPressSample = AbilityPressSample {
            ability_presses: AbilityPressSet::FIRST.with(AbilityPressSet::FOURTH),
            aim: Some(AIM),
        };

        assert_eq!(
            sample.masked(AbilityPressSet::FOURTH),
            AbilityPressSample {
                ability_presses: AbilityPressSet::FOURTH,
                aim: None,
            },
        );
        assert_eq!(sample.masked(AbilityPressSet::FIRST).aim, Some(AIM));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib play::game_key`

Expected: compile failure, first line ``error[E0425]: cannot find type `AimVector` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/play/game_key.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::collections::BTreeSet;

use crate::ability::{AbilityPressSet, AimVector};

const MOVEMENT_KEYS: [GameKeyKind; 4] = [
    GameKeyKind::MoveLeft,
    GameKeyKind::MoveUp,
    GameKeyKind::MoveRight,
    GameKeyKind::MoveDown,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GameKeyKind {
    MoveLeft,
    MoveUp,
    MoveRight,
    MoveDown,
    FirstAbility,
    SecondAbility,
    ThirdAbility,
    FourthAbility,
    Respawn,
    Pause,
    Fullscreen,
}

impl GameKeyKind {
    /// `None` for keys which press no ability.
    pub fn ability_press(self) -> Option<AbilityPressSet> {
        match self {
            GameKeyKind::FirstAbility => Some(AbilityPressSet::FIRST),
            GameKeyKind::SecondAbility => Some(AbilityPressSet::SECOND),
            GameKeyKind::ThirdAbility => Some(AbilityPressSet::THIRD),
            GameKeyKind::FourthAbility => Some(AbilityPressSet::FOURTH),
            GameKeyKind::MoveLeft
            | GameKeyKind::MoveUp
            | GameKeyKind::MoveRight
            | GameKeyKind::MoveDown
            | GameKeyKind::Respawn
            | GameKeyKind::Pause
            | GameKeyKind::Fullscreen => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyTransitionKind {
    Pressed,
    Released,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRepeatKind {
    Initial,
    Repeated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbilityPressSample {
    pub ability_presses: AbilityPressSet,
    /// Present only with a first or second press.
    pub aim: Option<AimVector>,
}

impl AbilityPressSample {
    pub const NONE: AbilityPressSample = AbilityPressSample {
        ability_presses: AbilityPressSet::NONE,
        aim: None,
    };

    pub fn masked(self, enabled_presses: AbilityPressSet) -> AbilityPressSample {
        let ability_presses: AbilityPressSet = self.ability_presses.intersection(enabled_presses);

        AbilityPressSample {
            ability_presses,
            aim: self.aim.filter(|_| ability_presses.has_shot_slot_press()),
        }
    }
}

/// -1, 0 or 1 per axis; y grows downwards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovementDirection {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyboardState {
    held_keys: BTreeSet<GameKeyKind>,
    /// Keydown edges since the last sample.
    pending_sample: AbilityPressSample,
    /// Mouse offset from the on-screen crosshair.
    pointer_aim: AimVector,
}

impl KeyboardState {
    pub fn new() -> KeyboardState {
        KeyboardState {
            held_keys: BTreeSet::new(),
            pending_sample: AbilityPressSample::NONE,
            pointer_aim: AimVector { x: 0, y: 0 },
        }
    }

    pub fn key_changed(&mut self, key: GameKeyKind, transition: KeyTransitionKind, repeat: KeyRepeatKind) {
        match transition {
            KeyTransitionKind::Pressed => self.key_pressed(key, repeat),
            KeyTransitionKind::Released => {
                self.held_keys.remove(&key);
            }
        }
    }

    pub fn aim_changed(&mut self, aim: AimVector) {
        self.pointer_aim = aim;
    }

    /// For a lost window focus, after which no keyup arrives.
    pub fn release_all_keys(&mut self) {
        self.held_keys.clear();
    }

    pub fn is_held(&self, key: GameKeyKind) -> bool {
        self.held_keys.contains(&key)
    }

    pub fn is_any_movement_key_held(&self) -> bool {
        MOVEMENT_KEYS.iter().any(|key| self.is_held(*key))
    }

    pub fn movement_direction(&self) -> MovementDirection {
        MovementDirection {
            x: self.get_axis_direction(GameKeyKind::MoveLeft, GameKeyKind::MoveRight),
            y: self.get_axis_direction(GameKeyKind::MoveUp, GameKeyKind::MoveDown),
        }
    }

    pub fn pending_ability_presses(&self) -> AbilityPressSet {
        self.pending_sample.ability_presses
    }

    pub fn take_ability_presses(&mut self) -> AbilityPressSample {
        let sample: AbilityPressSample = self.pending_sample;
        self.pending_sample = AbilityPressSample::NONE;

        sample
    }

    pub fn discard_ability_presses(&mut self) {
        self.pending_sample = AbilityPressSample::NONE;
    }

    fn key_pressed(&mut self, key: GameKeyKind, repeat: KeyRepeatKind) {
        self.held_keys.insert(key);

        if repeat == KeyRepeatKind::Repeated {
            return;
        }

        let Some(ability_press) = key.ability_press() else {
            return;
        };

        self.record_ability_press(ability_press);
    }

    fn record_ability_press(&mut self, ability_press: AbilityPressSet) {
        self.pending_sample.ability_presses = self.pending_sample.ability_presses.with(ability_press);

        if ability_press.has_shot_slot_press() {
            self.pending_sample.aim = Some(self.pointer_aim);
        }
    }

    fn get_axis_direction(&self, negative_key: GameKeyKind, positive_key: GameKeyKind) -> i32 {
        i32::from(self.is_held(positive_key)) - i32::from(self.is_held(negative_key))
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib play::game_key`

Expected: `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 343 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/lib.rs shared/src/play/mod.rs shared/src/play/game_key.rs
git commit -m "game keys and press edges"
```

### Task 5: Crosshair controller and player controls

**Files:**
- Create: `shared/src/play/crosshair.rs`
- Create: `shared/src/play/player_controls.rs`
- Modify: `shared/src/play/mod.rs`
- Test: inline `mod tests` in `shared/src/play/crosshair.rs` and `shared/src/play/player_controls.rs`

The crosshair moves every frame at the given speed, diagonals scaled by cos 45°, opposite keys cancelling; it records the cursor sent for each client tick in a 64-entry history and rebases on `CursorCorrected`, so a clamp burst over several ticks shifts it once; a correction past the history snaps it (client-web.md, client session, frame pacing and driving). `PlayerControls` is the pair the platform owns.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/play/mod.rs`:

```rust
pub mod crosshair;
pub mod game_key;
pub mod player_controls;

pub use crosshair::*;
pub use game_key::*;
pub use player_controls::*;
```

Create `shared/src/play/crosshair.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::{GameKeyKind, KeyRepeatKind, KeyTransitionKind};

    const ORIGIN: SubpixelPoint = SubpixelPoint { x: 0, y: 0 };

    fn hold_keys(keys: &[GameKeyKind]) -> KeyboardState {
        let mut keyboard: KeyboardState = KeyboardState::new();

        for key in keys {
            keyboard.key_changed(*key, KeyTransitionKind::Pressed, KeyRepeatKind::Initial);
        }

        keyboard
    }

    fn create_point(x: i32) -> SubpixelPoint {
        SubpixelPoint { x, y: 0 }
    }

    #[test]
    fn advance_moves_at_the_speed_along_one_axis() {
        let mut crosshair: CrosshairController = CrosshairController::new(ORIGIN);

        crosshair.advance(
            &hold_keys(&[GameKeyKind::MoveRight]),
            1000.0,
            CROSSHAIR_SPEED_PIXELS_PER_SECOND,
        );

        assert_eq!(crosshair.position(), SubpixelPoint { x: 43_520, y: 0 });
    }

    #[test]
    fn advance_scales_a_diagonal_by_cos_45() {
        let mut crosshair: CrosshairController = CrosshairController::new(ORIGIN);

        crosshair.advance(
            &hold_keys(&[GameKeyKind::MoveLeft, GameKeyKind::MoveDown]),
            1000.0,
            CROSSHAIR_SPEED_PIXELS_PER_SECOND,
        );

        assert_eq!(crosshair.position(), SubpixelPoint { x: -30_773, y: 30_773 });
    }

    #[test]
    fn advance_accumulates_movement_finer_than_a_subpixel() {
        let mut crosshair: CrosshairController = CrosshairController::new(ORIGIN);
        let keyboard: KeyboardState = hold_keys(&[GameKeyKind::MoveUp]);

        for _ in 0..4 {
            crosshair.advance(&keyboard, 250.0, SPECTATOR_CAMERA_SPEED_PIXELS_PER_SECOND);
        }

        assert_eq!(crosshair.position(), SubpixelPoint { x: 0, y: -64_000 });
    }

    #[test]
    fn advance_does_not_move_with_opposite_keys() {
        let mut crosshair: CrosshairController = CrosshairController::new(ORIGIN);

        crosshair.advance(
            &hold_keys(&[GameKeyKind::MoveLeft, GameKeyKind::MoveRight]),
            1000.0,
            CROSSHAIR_SPEED_PIXELS_PER_SECOND,
        );

        assert_eq!(crosshair.position(), ORIGIN);
    }

    #[test]
    fn advance_stops_at_the_coordinate_limit() {
        let mut crosshair: CrosshairController =
            CrosshairController::new(create_point(protocol_limits::COORDINATE_LIMIT_SUBPIXELS - 10));

        crosshair.advance(
            &hold_keys(&[GameKeyKind::MoveRight]),
            1000.0,
            CROSSHAIR_SPEED_PIXELS_PER_SECOND,
        );

        assert_eq!(
            crosshair.position(),
            create_point(protocol_limits::COORDINATE_LIMIT_SUBPIXELS),
        );
    }

    #[test]
    fn apply_correction_shifts_the_crosshair_once_for_a_clamp_burst() {
        let mut crosshair: CrosshairController = CrosshairController::new(create_point(17_000));
        crosshair.record_sent(Tick(10), create_point(10_000));
        crosshair.record_sent(Tick(11), create_point(13_000));
        crosshair.record_sent(Tick(12), create_point(16_000));

        crosshair.apply_correction(Tick(10), create_point(8000));

        assert_eq!(crosshair.position(), create_point(15_000));

        crosshair.apply_correction(Tick(11), create_point(11_000));
        crosshair.apply_correction(Tick(12), create_point(14_000));

        assert_eq!(crosshair.position(), create_point(15_000));
    }

    #[test]
    fn apply_correction_snaps_for_a_tick_no_longer_in_the_history() {
        let mut crosshair: CrosshairController = CrosshairController::new(ORIGIN);

        for tick in 0..=SENT_CURSOR_HISTORY_LENGTH {
            crosshair.record_sent(Tick(u32::try_from(tick).unwrap()), ORIGIN);
        }

        crosshair.apply_correction(Tick(0), create_point(-500));

        assert_eq!(crosshair.position(), create_point(-500));
    }

    #[test]
    fn snap_to_moves_the_crosshair_and_forgets_the_sent_cursors() {
        let mut crosshair: CrosshairController = CrosshairController::new(ORIGIN);
        crosshair.record_sent(Tick(3), create_point(100));

        crosshair.snap_to(create_point(9000));
        crosshair.apply_correction(Tick(3), create_point(50));

        assert_eq!(crosshair.position(), create_point(50));
    }
}
```

Create `shared/src/play/player_controls.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::AbilityPressSet;
    use crate::play::MovementDirection;

    #[test]
    fn new_holds_no_key_and_places_the_crosshair_at_the_origin() {
        let controls: PlayerControls = PlayerControls::new();

        assert_eq!(controls.keyboard.movement_direction(), MovementDirection { x: 0, y: 0 });
        assert_eq!(controls.keyboard.pending_ability_presses(), AbilityPressSet::NONE);
        assert_eq!(controls.crosshair.position(), SubpixelPoint { x: 0, y: 0 });
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib -- play::crosshair play::player_controls`

Expected: compile failure, first line ``error[E0425]: cannot find type `SubpixelPoint` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/play/crosshair.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::collections::VecDeque;
use std::f64::consts::FRAC_1_SQRT_2;

use crate::game::Tick;
use crate::geometry;
use crate::geometry::{SubpixelPoint, SubpixelVector};
use crate::play::{KeyboardState, MovementDirection};
use crate::protocol::protocol_limits;

pub const CROSSHAIR_SPEED_PIXELS_PER_SECOND: f64 = 42.5;
pub const SPECTATOR_CAMERA_SPEED_PIXELS_PER_SECOND: f64 = 62.5;
pub const SENT_CURSOR_HISTORY_LENGTH: usize = 64;
const MILLISECONDS_PER_SECOND: f64 = 1000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SentCursor {
    pub client_tick: Tick,
    pub cursor: SubpixelPoint,
}

/// The newest `SENT_CURSOR_HISTORY_LENGTH` sent cursors, oldest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentCursorHistory {
    sent_cursors: VecDeque<SentCursor>,
}

impl SentCursorHistory {
    pub fn new() -> SentCursorHistory {
        SentCursorHistory {
            sent_cursors: VecDeque::with_capacity(SENT_CURSOR_HISTORY_LENGTH),
        }
    }

    pub fn record(&mut self, client_tick: Tick, cursor: SubpixelPoint) {
        if self.sent_cursors.len() == SENT_CURSOR_HISTORY_LENGTH {
            self.sent_cursors.pop_front();
        }

        self.sent_cursors.push_back(SentCursor { client_tick, cursor });
    }

    pub fn get(&self, client_tick: Tick) -> Option<SubpixelPoint> {
        self.sent_cursors
            .iter()
            .find(|sent_cursor| sent_cursor.client_tick == client_tick)
            .map(|sent_cursor| sent_cursor.cursor)
    }

    /// Moves the cursor sent for `client_tick` and every later one.
    pub fn shift_from(&mut self, client_tick: Tick, shift: SubpixelVector) {
        for sent_cursor in self.sent_cursors.iter_mut() {
            if sent_cursor.client_tick >= client_tick {
                sent_cursor.cursor = get_shifted_point(sent_cursor.cursor, shift);
            }
        }
    }

    pub fn clear(&mut self) {
        self.sent_cursors.clear();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CrosshairController {
    /// Subpixels, unrounded, so frames shorter than a subpixel of movement still move it.
    position_x: f64,
    position_y: f64,
    sent_cursors: SentCursorHistory,
}

impl CrosshairController {
    pub fn new(position: SubpixelPoint) -> CrosshairController {
        CrosshairController {
            position_x: f64::from(position.x),
            position_y: f64::from(position.y),
            sent_cursors: SentCursorHistory::new(),
        }
    }

    pub fn position(&self) -> SubpixelPoint {
        SubpixelPoint {
            x: get_rounded_coordinate(self.position_x),
            y: get_rounded_coordinate(self.position_y),
        }
    }

    pub fn advance(
        &mut self,
        keyboard: &KeyboardState,
        frame_duration_milliseconds: f64,
        speed_pixels_per_second: f64,
    ) {
        let direction: MovementDirection = keyboard.movement_direction();
        let is_diagonal: bool = direction.x != 0 && direction.y != 0;
        let axis_scale: f64 = if is_diagonal { FRAC_1_SQRT_2 } else { 1.0 };
        let distance_subpixels: f64 =
            speed_pixels_per_second * f64::from(geometry::SUBPIXELS_PER_PIXEL) * frame_duration_milliseconds
                / MILLISECONDS_PER_SECOND
                * axis_scale;

        self.position_x = get_limited_coordinate(self.position_x + f64::from(direction.x) * distance_subpixels);
        self.position_y = get_limited_coordinate(self.position_y + f64::from(direction.y) * distance_subpixels);
    }

    /// Forgets the sent cursors, which the snap makes stale.
    pub fn snap_to(&mut self, position: SubpixelPoint) {
        self.position_x = f64::from(position.x);
        self.position_y = f64::from(position.y);
        self.sent_cursors.clear();
    }

    pub fn record_sent(&mut self, client_tick: Tick, cursor: SubpixelPoint) {
        self.sent_cursors.record(client_tick, cursor);
    }

    /// The server clamped the cursor sent for `client_tick` to `cursor`.
    pub fn apply_correction(&mut self, client_tick: Tick, cursor: SubpixelPoint) {
        let Some(sent_cursor) = self.sent_cursors.get(client_tick) else {
            self.snap_to(cursor);

            return;
        };

        let shift: SubpixelVector = SubpixelVector {
            x: cursor.x - sent_cursor.x,
            y: cursor.y - sent_cursor.y,
        };

        self.position_x = get_limited_coordinate(self.position_x + f64::from(shift.x));
        self.position_y = get_limited_coordinate(self.position_y + f64::from(shift.y));
        self.sent_cursors.shift_from(client_tick, shift);
    }
}

fn get_limited_coordinate(coordinate: f64) -> f64 {
    let limit: f64 = f64::from(protocol_limits::COORDINATE_LIMIT_SUBPIXELS);

    coordinate.clamp(-limit, limit)
}

/// Within the coordinate limit, so the conversion cannot saturate.
fn get_rounded_coordinate(coordinate: f64) -> i32 {
    get_limited_coordinate(coordinate).round() as i32
}

fn get_shifted_point(point: SubpixelPoint, shift: SubpixelVector) -> SubpixelPoint {
    SubpixelPoint {
        x: get_rounded_coordinate(f64::from(point.x) + f64::from(shift.x)),
        y: get_rounded_coordinate(f64::from(point.y) + f64::from(shift.y)),
    }
}
```

In `shared/src/play/player_controls.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::geometry::SubpixelPoint;
use crate::play::{CrosshairController, KeyboardState};

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerControls {
    pub keyboard: KeyboardState,
    pub crosshair: CrosshairController,
}

impl PlayerControls {
    pub fn new() -> PlayerControls {
        PlayerControls {
            keyboard: KeyboardState::new(),
            crosshair: CrosshairController::new(SubpixelPoint { x: 0, y: 0 }),
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib -- play::crosshair play::player_controls`

Expected: `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 352 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/play/mod.rs shared/src/play/crosshair.rs shared/src/play/player_controls.rs
git commit -m "crosshair controller and player controls"
```

### Task 6: Frame pacer

**Files:**
- Create: `shared/src/play/frame_pacing.rs`
- Modify: `shared/src/play/mod.rs`
- Test: inline `mod tests` in `shared/src/play/frame_pacing.rs`

Every scene paces its frames with a `FramePacer`: the frame duration clamped to 250 ms, local ticks from a 70 ms accumulator at most four per frame (the rest dropped), and the interpolation alpha, the time since the last applied tick over one tick period clamped to [0, 1]. `FrameOutput` joins in Task 11, with the effects it carries.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/play/mod.rs`:

```rust
pub mod crosshair;
pub mod frame_pacing;
pub mod game_key;
pub mod player_controls;

pub use crosshair::*;
pub use frame_pacing::*;
pub use game_key::*;
pub use player_controls::*;
```

Create `shared/src/play/frame_pacing.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_frame_gives_zero_for_the_first_frame_then_the_elapsed_time() {
        let mut pacer: FramePacer = FramePacer::new();

        assert_eq!(pacer.begin_frame(5000.0), 0.0);
        assert_eq!(pacer.begin_frame(5016.5), 16.5);
    }

    #[test]
    fn begin_frame_clamps_a_long_frame_to_the_frame_duration_limit() {
        let mut pacer: FramePacer = FramePacer::new();
        pacer.begin_frame(0.0);

        assert_eq!(pacer.begin_frame(9000.0), FRAME_DURATION_LIMIT_MILLISECONDS);
        assert_eq!(pacer.begin_frame(8000.0), 0.0);
    }

    #[test]
    fn take_local_tick_count_carries_the_remainder_to_the_next_frame() {
        let mut pacer: FramePacer = FramePacer::new();

        let local_tick_counts: Vec<u32> = [100.0, 50.0, 59.0, 1.0]
            .into_iter()
            .map(|frame_duration_milliseconds| pacer.take_local_tick_count(frame_duration_milliseconds))
            .collect();

        assert_eq!(local_tick_counts, vec![1, 1, 0, 1]);
    }

    #[test]
    fn take_local_tick_count_drops_the_ticks_beyond_the_per_frame_limit() {
        let mut pacer: FramePacer = FramePacer::new();

        assert_eq!(pacer.take_local_tick_count(1000.0), LOCAL_TICKS_PER_FRAME_LIMIT);
        assert_eq!(pacer.take_local_tick_count(49.0), 0);
        assert_eq!(pacer.take_local_tick_count(1.0), 1);
    }

    #[test]
    fn get_interpolation_alpha_is_the_proportion_of_a_tick_since_the_last_applied_tick() {
        let mut pacer: FramePacer = FramePacer::new();

        assert_eq!(pacer.get_interpolation_alpha(1000.0), 1.0);

        pacer.record_tick_applied(1000.0);

        assert_eq!(pacer.get_interpolation_alpha(1035.0), 0.5);
        assert_eq!(pacer.get_interpolation_alpha(2000.0), 1.0);
        assert_eq!(pacer.get_interpolation_alpha(900.0), 0.0);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib play::frame_pacing`

Expected: compile failure, first line ``error[E0425]: cannot find type `FramePacer` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/play/frame_pacing.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::game::tick;

pub const FRAME_DURATION_LIMIT_MILLISECONDS: f64 = 250.0;
pub const LOCAL_TICKS_PER_FRAME_LIMIT: u32 = 4;
pub const TICK_PERIOD_MILLISECONDS: f64 = tick::TICK_PERIOD_MILLISECONDS as f64;

#[derive(Clone, Debug, PartialEq)]
pub struct FramePacer {
    previous_frame_at_milliseconds: Option<f64>,
    local_tick_accumulator_milliseconds: f64,
    tick_applied_at_milliseconds: Option<f64>,
}

impl FramePacer {
    pub fn new() -> FramePacer {
        FramePacer {
            previous_frame_at_milliseconds: None,
            local_tick_accumulator_milliseconds: 0.0,
            tick_applied_at_milliseconds: None,
        }
    }

    /// Zero for the first frame and for a clock which went backwards.
    pub fn begin_frame(&mut self, now_milliseconds: f64) -> f64 {
        let frame_duration_milliseconds: f64 = match self.previous_frame_at_milliseconds {
            Some(previous_frame_at_milliseconds) => {
                (now_milliseconds - previous_frame_at_milliseconds).clamp(0.0, FRAME_DURATION_LIMIT_MILLISECONDS)
            }
            None => 0.0,
        };

        self.previous_frame_at_milliseconds = Some(now_milliseconds);

        frame_duration_milliseconds
    }

    /// Ticks due beyond the per-frame limit are dropped; the part of a tick not yet due carries over.
    pub fn take_local_tick_count(&mut self, frame_duration_milliseconds: f64) -> u32 {
        self.local_tick_accumulator_milliseconds += frame_duration_milliseconds;

        let due_tick_count: f64 = (self.local_tick_accumulator_milliseconds / TICK_PERIOD_MILLISECONDS).floor();
        self.local_tick_accumulator_milliseconds -= due_tick_count * TICK_PERIOD_MILLISECONDS;

        (due_tick_count as u32).min(LOCAL_TICKS_PER_FRAME_LIMIT)
    }

    pub fn record_tick_applied(&mut self, now_milliseconds: f64) {
        self.tick_applied_at_milliseconds = Some(now_milliseconds);
    }

    /// 1 before the first applied tick.
    pub fn get_interpolation_alpha(&self, now_milliseconds: f64) -> f32 {
        let Some(tick_applied_at_milliseconds) = self.tick_applied_at_milliseconds else {
            return 1.0;
        };

        let alpha: f64 = (now_milliseconds - tick_applied_at_milliseconds) / TICK_PERIOD_MILLISECONDS;

        alpha.clamp(0.0, 1.0) as f32
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib play::frame_pacing`

Expected: `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 361 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/play/mod.rs shared/src/play/frame_pacing.rs
git commit -m "frame pacer"
```

### Task 7: Projectile interpolation

**Files:**
- Create: `shared/src/play/interpolation.rs`
- Modify: `shared/src/play/mod.rs`
- Test: inline `mod tests` in `shared/src/play/interpolation.rs`

Projectiles are drawn between their previous and current tick positions (client-web.md, frame pacing). A flying projectile moved by exactly its velocity in the last applied tick, launched ones included (simulation.md, tick order, phase 5), so its previous position is one velocity back and no previous state is kept.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/play/mod.rs`:

```rust
pub mod crosshair;
pub mod frame_pacing;
pub mod game_key;
pub mod interpolation;
pub mod player_controls;

pub use crosshair::*;
pub use frame_pacing::*;
pub use game_key::*;
pub use interpolation::*;
pub use player_controls::*;
```

Create `shared/src/play/interpolation.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::SubpixelVector;

    const PROJECTILE: Projectile = Projectile {
        position: SubpixelPoint { x: 10_240, y: 0 },
        velocity: SubpixelVector { x: 1024, y: -2048 },
    };

    #[test]
    fn get_flying_projectile_position_moves_from_the_previous_tick_position_to_the_current_one() {
        let positions: Vec<InterpolatedPoint> = [0.0, 0.5, 1.0]
            .into_iter()
            .map(|alpha| InterpolationFrame { alpha }.get_flying_projectile_position(&PROJECTILE))
            .collect();

        assert_eq!(
            positions,
            vec![
                InterpolatedPoint { x: 9.0, y: 2.0 },
                InterpolatedPoint { x: 9.5, y: 1.0 },
                InterpolatedPoint { x: 10.0, y: 0.0 },
            ],
        );
    }

    #[test]
    fn from_subpixel_point_converts_to_unrounded_pixels() {
        assert_eq!(
            InterpolatedPoint::from_subpixel_point(SubpixelPoint { x: 1536, y: -256 }),
            InterpolatedPoint { x: 1.5, y: -0.25 },
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib play::interpolation`

Expected: compile failure, first line ``error[E0425]: cannot find type `Projectile` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/play/interpolation.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::ability::Projectile;
use crate::geometry;
use crate::geometry::SubpixelPoint;

const SUBPIXELS_PER_PIXEL: f64 = geometry::SUBPIXELS_PER_PIXEL as f64;

/// World pixels, unrounded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InterpolatedPoint {
    pub x: f32,
    pub y: f32,
}

impl InterpolatedPoint {
    pub fn from_subpixel_point(point: SubpixelPoint) -> InterpolatedPoint {
        InterpolatedPoint {
            x: (f64::from(point.x) / SUBPIXELS_PER_PIXEL) as f32,
            y: (f64::from(point.y) / SUBPIXELS_PER_PIXEL) as f32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InterpolationFrame {
    /// 0 at the last applied tick, 1 one tick period later.
    pub alpha: f32,
}

impl InterpolationFrame {
    /// A flying projectile moved by exactly its velocity in the last applied tick.
    pub fn get_flying_projectile_position(&self, projectile: &Projectile) -> InterpolatedPoint {
        let remaining_proportion: f64 = 1.0 - f64::from(self.alpha);

        InterpolatedPoint {
            x: get_interpolated_pixels(projectile.position.x, projectile.velocity.x, remaining_proportion),
            y: get_interpolated_pixels(projectile.position.y, projectile.velocity.y, remaining_proportion),
        }
    }
}

fn get_interpolated_pixels(position: i32, velocity: i32, remaining_proportion: f64) -> f32 {
    let subpixels: f64 = f64::from(position) - f64::from(velocity) * remaining_proportion;

    (subpixels / SUBPIXELS_PER_PIXEL) as f32
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib play::interpolation`

Expected: `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 366 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/play/mod.rs shared/src/play/interpolation.rs
git commit -m "projectile interpolation"
```

### Task 8: Own member state

**Files:**
- Create: `shared/src/play/own_member_state.rs`
- Modify: `shared/src/play/mod.rs`
- Test: inline `mod tests` in `shared/src/play/own_member_state.rs`

The own member is an alive Participant (crosshair at 42.5 px/s), a dead Participant or a Spectator (camera at 62.5 px/s). R opens the respawn menu for a member without an organism when the round allows spawning and alive organisms are below the cap; at the cap the platform shows "Game is at maximum player capacity" (client-web.md, client session, frame pacing and driving).

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/play/mod.rs`:

```rust
pub mod crosshair;
pub mod frame_pacing;
pub mod game_key;
pub mod interpolation;
pub mod own_member_state;
pub mod player_controls;

pub use crosshair::*;
pub use frame_pacing::*;
pub use game_key::*;
pub use interpolation::*;
pub use own_member_state::*;
pub use player_controls::*;
```

Create `shared/src/play/own_member_state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, Tick, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::round::RoundState;
    use crate::world::WorldShapeKind;

    const OWN_MEMBER_ID: MemberId = MemberId(0);
    const OTHER_MEMBER_ID: MemberId = MemberId(1);

    fn create_spectator(member_id: MemberId) -> Member {
        let mut member: Member = test_fixture::create_participant(member_id);
        member.role = MemberRoleKind::Spectator;
        member.loadout = None;

        member
    }

    fn create_state_with_own_member(mode: GameModeKind, own_member: Member) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);
        state.members.insert(OWN_MEMBER_ID, own_member);

        state
    }

    fn set_round_phase(state: &mut GameState, phase: RoundPhase) {
        state.round = Some(RoundState {
            phase,
            phase_started_at: Tick(0),
        });
    }

    #[test]
    fn from_member_classifies_alive_dead_and_spectating_members() {
        let alive_member: Member =
            test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 100, y: 100 });

        assert_eq!(
            OwnMemberStateKind::from_member(&alive_member),
            OwnMemberStateKind::AliveParticipant,
        );
        assert_eq!(
            OwnMemberStateKind::from_member(&test_fixture::create_participant(OWN_MEMBER_ID)),
            OwnMemberStateKind::DeadParticipant,
        );
        assert_eq!(
            OwnMemberStateKind::from_member(&create_spectator(OWN_MEMBER_ID)),
            OwnMemberStateKind::Spectator,
        );
    }

    #[test]
    fn get_own_member_state_is_none_before_the_member_joins() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);

        assert_eq!(get_own_member_state(&state, OWN_MEMBER_ID), None);
    }

    #[test]
    fn crosshair_speed_pixels_per_second_moves_a_camera_faster_than_a_crosshair() {
        assert_eq!(
            OwnMemberStateKind::AliveParticipant.crosshair_speed_pixels_per_second(),
            42.5,
        );
        assert_eq!(
            OwnMemberStateKind::DeadParticipant.crosshair_speed_pixels_per_second(),
            62.5,
        );
        assert_eq!(OwnMemberStateKind::Spectator.crosshair_speed_pixels_per_second(), 62.5);
    }

    #[test]
    fn get_respawn_key_outcome_ignores_an_alive_participant() {
        let state: GameState = create_state_with_own_member(
            GameModeKind::FreeForAll,
            test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 100, y: 100 }),
        );

        assert_eq!(
            get_respawn_key_outcome(&state, OWN_MEMBER_ID),
            RespawnKeyOutcomeKind::Ignore,
        );
    }

    #[test]
    fn get_respawn_key_outcome_opens_the_respawn_menu_below_the_cap() {
        let state: GameState = create_state_with_own_member(
            GameModeKind::FreeForAll,
            test_fixture::create_participant(OWN_MEMBER_ID),
        );

        assert_eq!(
            get_respawn_key_outcome(&state, OWN_MEMBER_ID),
            RespawnKeyOutcomeKind::OpenRespawnMenu,
        );
    }

    #[test]
    fn get_respawn_key_outcome_shows_the_capacity_message_at_the_cap() {
        let mut state: GameState =
            create_state_with_own_member(GameModeKind::Skirmish, create_spectator(OWN_MEMBER_ID));
        state.settings.player_cap = 1;
        state.members.insert(
            OTHER_MEMBER_ID,
            test_fixture::create_participant_with_organism(OTHER_MEMBER_ID, WorldPoint { x: 100, y: 100 }),
        );

        assert_eq!(
            get_respawn_key_outcome(&state, OWN_MEMBER_ID),
            RespawnKeyOutcomeKind::ShowGameAtCapacity,
        );
    }

    #[test]
    fn get_respawn_key_outcome_allows_spawning_only_before_a_round_plays() {
        let mut state: GameState =
            create_state_with_own_member(GameModeKind::Survival, test_fixture::create_participant(OWN_MEMBER_ID));
        let mut outcomes: Vec<RespawnKeyOutcomeKind> = Vec::new();

        for phase in [
            RoundPhase::Waiting,
            RoundPhase::PreRound,
            RoundPhase::Playing,
            RoundPhase::PostRound,
        ] {
            set_round_phase(&mut state, phase);
            outcomes.push(get_respawn_key_outcome(&state, OWN_MEMBER_ID));
        }

        assert_eq!(
            outcomes,
            vec![
                RespawnKeyOutcomeKind::OpenRespawnMenu,
                RespawnKeyOutcomeKind::OpenRespawnMenu,
                RespawnKeyOutcomeKind::Ignore,
                RespawnKeyOutcomeKind::Ignore,
            ],
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib play::own_member_state`

Expected: compile failure, first line ``error[E0425]: cannot find type `MemberId` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/play/own_member_state.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::game::GameState;
use crate::member::{Member, MemberId, MemberRoleKind};
use crate::play::crosshair;
use crate::round::RoundPhase;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnMemberStateKind {
    AliveParticipant,
    DeadParticipant,
    Spectator,
}

impl OwnMemberStateKind {
    pub fn from_member(member: &Member) -> OwnMemberStateKind {
        match (&member.organism, member.role) {
            (Some(_), _) => OwnMemberStateKind::AliveParticipant,
            (None, MemberRoleKind::Participant) => OwnMemberStateKind::DeadParticipant,
            (None, MemberRoleKind::Spectator) => OwnMemberStateKind::Spectator,
        }
    }

    /// Without an organism the crosshair is the spectating camera.
    pub fn crosshair_speed_pixels_per_second(self) -> f64 {
        match self {
            OwnMemberStateKind::AliveParticipant => crosshair::CROSSHAIR_SPEED_PIXELS_PER_SECOND,
            OwnMemberStateKind::DeadParticipant | OwnMemberStateKind::Spectator => {
                crosshair::SPECTATOR_CAMERA_SPEED_PIXELS_PER_SECOND
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RespawnKeyOutcomeKind {
    OpenRespawnMenu,
    ShowGameAtCapacity,
    Ignore,
}

/// `None` until the member's `Joined` has been applied.
pub fn get_own_member_state(state: &GameState, member_id: MemberId) -> Option<OwnMemberStateKind> {
    state.members.get(&member_id).map(OwnMemberStateKind::from_member)
}

pub fn get_respawn_key_outcome(state: &GameState, member_id: MemberId) -> RespawnKeyOutcomeKind {
    let own_member_state: Option<OwnMemberStateKind> = get_own_member_state(state, member_id);
    let is_without_organism: bool = matches!(
        own_member_state,
        Some(OwnMemberStateKind::DeadParticipant | OwnMemberStateKind::Spectator)
    );

    if !is_without_organism || !is_spawn_allowed_by_round(state) {
        return RespawnKeyOutcomeKind::Ignore;
    }

    let is_at_player_cap: bool = state.alive_organism_count() >= u32::from(state.settings.player_cap);

    if is_at_player_cap {
        return RespawnKeyOutcomeKind::ShowGameAtCapacity;
    }

    RespawnKeyOutcomeKind::OpenRespawnMenu
}

fn is_spawn_allowed_by_round(state: &GameState) -> bool {
    state.round.is_none_or(|round| matches!(round.phase, RoundPhase::Waiting | RoundPhase::PreRound))
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib play::own_member_state`

Expected: `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 368 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/play/mod.rs shared/src/play/own_member_state.rs
git commit -m "own member state"
```

### Task 9: Message text

**Files:**
- Create: `shared/src/play/message_text.rs`
- Modify: `shared/src/play/mod.rs`
- Test: inline `mod tests` in `shared/src/play/message_text.rs`

The message box texts per mode, round phase and own state, countdowns from ticks (client-web.md, overlays), and the transient texts which replace them for 3 s. "Wait for the round to complete" and "Game is at maximum player capacity" are the texts of `RejectionKind::RoundInProgress` and `RejectionKind::GameFull`, reused.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/play/mod.rs`:

```rust
pub mod crosshair;
pub mod frame_pacing;
pub mod game_key;
pub mod interpolation;
pub mod message_text;
pub mod own_member_state;
pub mod player_controls;

pub use crosshair::*;
pub use frame_pacing::*;
pub use game_key::*;
pub use interpolation::*;
pub use message_text::*;
pub use own_member_state::*;
pub use player_controls::*;
```

Create `shared/src/play/message_text.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, Tick, test_fixture};
    use crate::geometry::WorldPoint;
    use crate::member::{Member, MemberRoleKind};
    use crate::round::RoundState;
    use crate::world::WorldShapeKind;

    const OWN_MEMBER_ID: MemberId = MemberId(0);

    fn create_state(mode: GameModeKind, own_member: Member) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);
        state.members.insert(OWN_MEMBER_ID, own_member);

        state
    }

    fn create_alive_member() -> Member {
        test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 100, y: 100 })
    }

    fn create_spectator() -> Member {
        let mut member: Member = test_fixture::create_participant(OWN_MEMBER_ID);
        member.role = MemberRoleKind::Spectator;
        member.loadout = None;

        member
    }

    fn create_survival_state(own_member: Member, phase: RoundPhase, tick: Tick) -> GameState {
        let mut state: GameState = create_state(GameModeKind::Survival, own_member);
        state.round = Some(RoundState {
            phase,
            phase_started_at: Tick(0),
        });
        state.tick = tick;

        state
    }

    fn get_own_message_text(state: &GameState) -> Option<String> {
        get_message_text(state, OWN_MEMBER_ID)
    }

    #[test]
    fn get_message_text_without_rounds_asks_a_member_without_an_organism_to_spawn() {
        let alive_state: GameState = create_state(GameModeKind::FreeForAll, create_alive_member());
        let dead_state: GameState =
            create_state(GameModeKind::Skirmish, test_fixture::create_participant(OWN_MEMBER_ID));
        let spectator_state: GameState = create_state(GameModeKind::FreeForAll, create_spectator());

        assert_eq!(get_own_message_text(&alive_state), None);
        assert_eq!(
            get_own_message_text(&dead_state),
            Some(String::from("Press 'R' to Spawn")),
        );
        assert_eq!(
            get_own_message_text(&spectator_state),
            Some(String::from("Press 'R' to Spawn")),
        );
    }

    #[test]
    fn get_message_text_while_waiting_counts_the_missing_players() {
        let singular_state: GameState = create_survival_state(create_alive_member(), RoundPhase::Waiting, Tick(5));
        let plural_state: GameState = create_survival_state(create_spectator(), RoundPhase::Waiting, Tick(5));

        assert_eq!(
            get_own_message_text(&singular_state),
            Some(String::from("Waiting for 1 more player to join")),
        );
        assert_eq!(
            get_own_message_text(&plural_state),
            Some(String::from("Waiting for 2 more players to join")),
        );
    }

    #[test]
    fn get_message_text_counts_down_before_and_after_a_round() {
        let pre_round_state: GameState = create_survival_state(create_alive_member(), RoundPhase::PreRound, Tick(14));
        let post_round_state: GameState = create_survival_state(create_spectator(), RoundPhase::PostRound, Tick(0));

        assert_eq!(
            get_own_message_text(&pre_round_state),
            Some(String::from("Round begins in: 7")),
        );
        assert_eq!(
            get_own_message_text(&post_round_state),
            Some(String::from("Round ends in: 8")),
        );
    }

    #[test]
    fn get_message_text_while_playing_asks_only_a_member_without_an_organism_to_wait() {
        let alive_state: GameState = create_survival_state(create_alive_member(), RoundPhase::Playing, Tick(5));
        let dead_state: GameState = create_survival_state(
            test_fixture::create_participant(OWN_MEMBER_ID),
            RoundPhase::Playing,
            Tick(5),
        );

        assert_eq!(get_own_message_text(&alive_state), None);
        assert_eq!(
            get_own_message_text(&dead_state),
            Some(String::from("Wait for the round to complete")),
        );
    }

    #[test]
    fn get_message_text_is_none_for_a_member_not_in_the_game() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);

        assert_eq!(get_own_message_text(&state), None);
    }

    #[test]
    fn text_gives_the_transient_message_texts() {
        assert_eq!(
            TransientMessageKind::NoRoomToSpawn.text(),
            "No room to spawn; press 'R' to try again",
        );
        assert_eq!(
            TransientMessageKind::GameAtCapacity.text(),
            "Game is at maximum player capacity",
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib play::message_text`

Expected: compile failure, first line ``error[E0425]: cannot find type `MemberId` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/play/message_text.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::game::GameState;
use crate::member::MemberId;
use crate::protocol::RejectionKind;
use crate::round;
use crate::round::{RoundPhase, RoundState};

pub const TRANSIENT_MESSAGE_DURATION_MILLISECONDS: f64 = 3000.0;
/// While a resync waits for its snapshot.
pub const CATCHING_UP_TEXT: &str = "Catching up";
const PRESS_TO_SPAWN_TEXT: &str = "Press 'R' to Spawn";
const NO_ROOM_TO_SPAWN_TEXT: &str = "No room to spawn; press 'R' to try again";

/// Shown for `TRANSIENT_MESSAGE_DURATION_MILLISECONDS` in place of the game's message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransientMessageKind {
    NoRoomToSpawn,
    GameAtCapacity,
}

impl TransientMessageKind {
    pub fn text(self) -> String {
        match self {
            TransientMessageKind::NoRoomToSpawn => String::from(NO_ROOM_TO_SPAWN_TEXT),
            TransientMessageKind::GameAtCapacity => RejectionKind::GameFull.message_text(),
        }
    }
}

/// `None` when there is nothing to tell the member.
pub fn get_message_text(state: &GameState, own_member_id: MemberId) -> Option<String> {
    let own_member_has_organism: bool = state.members.get(&own_member_id)?.organism.is_some();

    match state.round {
        Some(round) => get_round_message_text(state, round, own_member_has_organism),
        None => (!own_member_has_organism).then(|| String::from(PRESS_TO_SPAWN_TEXT)),
    }
}

fn get_round_message_text(state: &GameState, round: RoundState, own_member_has_organism: bool) -> Option<String> {
    match round.phase {
        RoundPhase::Waiting => Some(get_waiting_text(state)),
        RoundPhase::PreRound => round
            .get_countdown_seconds(state.tick)
            .map(|countdown_seconds| format!("Round begins in: {countdown_seconds}")),
        RoundPhase::Playing => (!own_member_has_organism).then(|| RejectionKind::RoundInProgress.message_text()),
        RoundPhase::PostRound => round
            .get_countdown_seconds(state.tick)
            .map(|countdown_seconds| format!("Round ends in: {countdown_seconds}")),
    }
}

fn get_waiting_text(state: &GameState) -> String {
    let player_minimum: u32 = u32::from(state.settings.player_minimum.unwrap_or(0));
    let missing_player_count: u32 = player_minimum.saturating_sub(round::count_participants(state));

    if missing_player_count == 1 {
        return String::from("Waiting for 1 more player to join");
    }

    format!("Waiting for {missing_player_count} more players to join")
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib play::message_text`

Expected: `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 375 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/play/mod.rs shared/src/play/message_text.rs
git commit -m "message text"
```

### Task 10: Client session frames

**Files:**
- Create: `shared/src/play/client_session.rs`
- Modify: `shared/src/play/mod.rs`
- Test: inline `mod tests` in `shared/src/play/client_session.rs`

The sans-IO session (client-web.md, client session, frame pacing and driving): it decodes each server frame and converts it with `TryFrom`; passes on game lists and rejections; enters a game on `JoinAccepted`; applies the snapshot and the bundles after it; reports the own member state, the own spawn and a rejected own spawn, and snaps the crosshair to the own spawn; after each applied bundle samples the controls into an `Input`, only while the own member has an organism; applies `CursorCorrected`. In this task an undecodable or unconvertible game frame or a bundle which does not step is logged and dropped; Task 11 turns each into a resync.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/play/mod.rs`:

```rust
pub mod client_session;
pub mod crosshair;
pub mod frame_pacing;
pub mod game_key;
pub mod interpolation;
pub mod message_text;
pub mod own_member_state;
pub mod player_controls;

pub use client_session::*;
pub use crosshair::*;
pub use frame_pacing::*;
pub use game_key::*;
pub use interpolation::*;
pub use message_text::*;
pub use own_member_state::*;
pub use player_controls::*;
```

Create `shared/src/play/client_session.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::AbilityPressSet;
    use crate::game::{GameModeKind, test_fixture};
    use crate::geometry;
    use crate::member::Member;
    use crate::play::{GameKeyKind, KeyRepeatKind, KeyTransitionKind};
    use crate::protocol::{
        GameModeKindSerial, GameStateSerialOut, LoadoutSerial, MemberEventSerialOut, MemberRoleKindSerialOut,
        RejectionKindSerialOut, RequestKindSerialOut, SettingFieldKind, SettingFieldKindSerialOut,
    };
    use crate::world::WorldShapeKind;

    const GAME_ID: GameId = GameId(7);
    const OWN_MEMBER_ID: MemberId = MemberId(2);
    const NOW_MILLISECONDS: f64 = 1000.0;

    const OTHER_MEMBER_ID: MemberId = MemberId(1);

    fn receive_message(session: &mut ClientSession, message_serial_out: &MessageSerialOut) -> Vec<ClientEffect> {
        let mut controls: PlayerControls = PlayerControls::new();

        receive_with_controls(session, message_serial_out, &mut controls)
    }

    fn receive_with_controls(
        session: &mut ClientSession,
        message_serial_out: &MessageSerialOut,
        controls: &mut PlayerControls,
    ) -> Vec<ClientEffect> {
        let frame: Vec<u8> = protocol::encode_message_out(message_serial_out);

        session.receive(&frame, controls, NOW_MILLISECONDS)
    }

    /// The tick-0 state of a new game, as the server sends it to a creator.
    fn create_server_state() -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        state.next_member_id = OWN_MEMBER_ID;

        state
    }

    fn create_server_state_with_own_member(own_member: Member) -> GameState {
        let mut state: GameState = create_server_state();
        state.members.insert(OWN_MEMBER_ID, own_member);
        state.next_member_id = OWN_MEMBER_ID.next();

        state
    }

    fn create_alive_own_member() -> Member {
        test_fixture::create_participant_with_organism(OWN_MEMBER_ID, WorldPoint { x: 300, y: 200 })
    }

    fn create_snapshot_message(state: &GameState) -> MessageSerialOut {
        MessageSerialOut::Snapshot(GameSnapshotSerialOut {
            game_id: GAME_ID.0,
            state: GameStateSerialOut::from(state),
        })
    }

    fn create_bundle_message(tick: Tick, member_event_serial_outs: Vec<MemberEventSerialOut>) -> MessageSerialOut {
        MessageSerialOut::InputBundle(InputBundleSerialOut {
            tick: tick.0,
            member_events: member_event_serial_outs,
            player_inputs: Vec::new(),
        })
    }

    fn create_own_join_events(role: MemberRoleKindSerialOut) -> Vec<MemberEventSerialOut> {
        let loadout_serial: LoadoutSerial = LoadoutSerial::from(&test_fixture::create_loadout());
        let joined_event_serial_out: MemberEventSerialOut = MemberEventSerialOut::Joined {
            member_id: OWN_MEMBER_ID.0,
            screen_name: String::from("Blob"),
            role,
            loadout: (role == MemberRoleKindSerialOut::Participant).then_some(loadout_serial),
            team: None,
        };

        match role {
            MemberRoleKindSerialOut::Participant => vec![
                joined_event_serial_out,
                MemberEventSerialOut::SpawnRequested {
                    member_id: OWN_MEMBER_ID.0,
                    loadout: loadout_serial,
                    team: None,
                },
            ],
            MemberRoleKindSerialOut::Spectator => vec![joined_event_serial_out],
        }
    }

    /// Joined, with the snapshot applied.
    fn create_session_with_snapshot(state: &GameState, controls: &mut PlayerControls) -> ClientSession {
        let mut session: ClientSession = create_joined_session();
        receive_with_controls(&mut session, &create_snapshot_message(state), controls);

        session
    }

    fn get_sent_messages(effects: &[ClientEffect]) -> Vec<MessageSerialIn> {
        effects
            .iter()
            .filter_map(|effect| match effect {
                ClientEffect::SendFrame(frame) => Some(protocol::decode_message_in(frame).unwrap()),
                _ => None,
            })
            .collect()
    }

    fn press(controls: &mut PlayerControls, key: GameKeyKind) {
        controls.keyboard.key_changed(key, KeyTransitionKind::Pressed, KeyRepeatKind::Initial);
    }

    fn create_joined_session() -> ClientSession {
        let mut session: ClientSession = ClientSession::new();
        receive_message(
            &mut session,
            &MessageSerialOut::JoinAccepted {
                game_id: GAME_ID.0,
                member_id: OWN_MEMBER_ID.0,
            },
        );

        session
    }

    fn create_game_summary_serial_out(title: &str) -> GameSummarySerialOut {
        GameSummarySerialOut {
            game_id: GAME_ID.0,
            title: String::from(title),
            mode: GameModeKindSerial::FreeForAll,
            player_count: 3,
            spectator_count: 1,
            player_cap: 8,
            secured: true,
            team_sizes: Vec::new(),
        }
    }

    #[test]
    fn receive_converts_a_game_list() {
        let mut session: ClientSession = ClientSession::new();

        let effects: Vec<ClientEffect> = receive_message(
            &mut session,
            &MessageSerialOut::GameList {
                game_summaries: vec![create_game_summary_serial_out("Arena")],
                online_client_count: 12,
            },
        );

        assert_eq!(
            effects,
            vec![ClientEffect::GameListReceived {
                game_summaries: vec![GameSummary {
                    game_id: GAME_ID,
                    title: String::from("Arena"),
                    mode: GameModeKind::FreeForAll,
                    player_count: 3,
                    spectator_count: 1,
                    player_cap: 8,
                    secured: true,
                    team_sizes: Vec::new(),
                }],
                online_client_count: 12,
            }],
        );
    }

    #[test]
    fn receive_drops_a_game_list_with_an_unconvertible_summary() {
        let mut session: ClientSession = ClientSession::new();

        let effects: Vec<ClientEffect> = receive_message(
            &mut session,
            &MessageSerialOut::GameList {
                game_summaries: vec![
                    create_game_summary_serial_out("Arena"),
                    create_game_summary_serial_out(""),
                ],
                online_client_count: 12,
            },
        );

        assert_eq!(effects, Vec::<ClientEffect>::new());
    }

    #[test]
    fn receive_enters_the_game_on_join_accepted() {
        let mut session: ClientSession = ClientSession::new();

        let effects: Vec<ClientEffect> = receive_message(
            &mut session,
            &MessageSerialOut::JoinAccepted {
                game_id: GAME_ID.0,
                member_id: OWN_MEMBER_ID.0,
            },
        );

        assert_eq!(
            effects,
            vec![ClientEffect::JoinAccepted {
                game_id: GAME_ID,
                member_id: OWN_MEMBER_ID,
            }],
        );
        assert_eq!(session.game_id(), Some(GAME_ID));
        assert_eq!(session.own_member_id(), Some(OWN_MEMBER_ID));
    }

    #[test]
    fn receive_passes_on_a_rejected_request() {
        let mut session: ClientSession = ClientSession::new();

        let effects: Vec<ClientEffect> = receive_message(
            &mut session,
            &MessageSerialOut::RequestRejected {
                request: RequestKindSerialOut::CreateGame,
                reason: RejectionKindSerialOut::SettingNotApplicable {
                    field: SettingFieldKindSerialOut::TeamCount,
                },
            },
        );

        assert_eq!(
            effects,
            vec![ClientEffect::RequestRejected {
                request: RequestKind::CreateGame,
                reason: RejectionKind::SettingNotApplicable {
                    field: SettingFieldKind::TeamCount,
                },
            }],
        );
    }

    #[test]
    fn receive_returns_to_the_lobby_when_the_game_is_left_or_ended() {
        let mut left_session: ClientSession = create_joined_session();
        let mut ended_session: ClientSession = create_joined_session();

        let left_effects: Vec<ClientEffect> = receive_message(&mut left_session, &MessageSerialOut::LeftGame);
        let ended_effects: Vec<ClientEffect> = receive_message(&mut ended_session, &MessageSerialOut::GameEnded);

        assert_eq!(left_effects, vec![ClientEffect::LeftGame]);
        assert_eq!(ended_effects, vec![ClientEffect::GameEnded]);
        assert_eq!(left_session.own_member_id(), None);
        assert_eq!(ended_session.game_id(), None);
    }

    #[test]
    fn receive_ignores_left_game_in_the_lobby() {
        let mut session: ClientSession = ClientSession::new();

        assert_eq!(
            receive_message(&mut session, &MessageSerialOut::LeftGame),
            Vec::<ClientEffect>::new(),
        );
    }

    #[test]
    fn receive_drops_an_undecodable_frame_in_the_lobby() {
        let mut session: ClientSession = ClientSession::new();
        let mut controls: PlayerControls = PlayerControls::new();

        let effects: Vec<ClientEffect> = session.receive(&[0xff, 0xff, 0xff], &mut controls, NOW_MILLISECONDS);

        assert_eq!(effects, Vec::<ClientEffect>::new());
    }

    #[test]
    fn send_request_encodes_the_request() {
        let mut session: ClientSession = ClientSession::new();

        let effects: Vec<ClientEffect> = session.send_request(MessageSerialIn::SubscribeGameList);

        assert_eq!(
            effects,
            vec![ClientEffect::SendFrame(protocol::encode_message_in(
                &MessageSerialIn::SubscribeGameList
            ))],
        );
    }

    #[test]
    fn connection_closed_reports_the_close_reason_and_leaves_the_game() {
        let mut session: ClientSession = create_joined_session();

        assert_eq!(
            session.connection_closed(4002),
            vec![ClientEffect::ConnectionLost(Some(CloseReasonKind::SlowConsumer))],
        );
        assert_eq!(session.own_member_id(), None);
        assert_eq!(
            session.connection_closed(1006),
            vec![ClientEffect::ConnectionLost(None)],
        );
    }

    #[test]
    fn receive_applies_the_snapshot_then_the_bundles_after_it() {
        let mut controls: PlayerControls = PlayerControls::new();
        let mut server_state: GameState = create_server_state();
        let mut session: ClientSession = create_session_with_snapshot(&server_state, &mut controls);
        let bundle_messages: Vec<MessageSerialOut> = vec![
            create_bundle_message(Tick(1), create_own_join_events(MemberRoleKindSerialOut::Participant)),
            create_bundle_message(Tick(2), Vec::new()),
        ];
        let mut applied_ticks: Vec<Tick> = Vec::new();

        for bundle_message in &bundle_messages {
            let MessageSerialOut::InputBundle(bundle_serial_out) = bundle_message else {
                unreachable!();
            };

            game::step(
                &mut server_state,
                &InputBundle::try_from(bundle_serial_out.clone()).unwrap(),
            )
            .unwrap();

            let effects: Vec<ClientEffect> = receive_with_controls(&mut session, bundle_message, &mut controls);
            applied_ticks.extend(effects.iter().filter_map(|effect| match effect {
                ClientEffect::TickApplied { tick } => Some(*tick),
                _ => None,
            }));
        }

        assert_eq!(applied_ticks, vec![Tick(1), Tick(2)]);
        assert_eq!(session.render_state(), Some(&server_state));
    }

    #[test]
    fn receive_places_the_crosshair_at_the_world_center_on_the_first_snapshot() {
        let mut controls: PlayerControls = PlayerControls::new();

        create_session_with_snapshot(&create_server_state(), &mut controls);

        let center_subpixels: i32 = 400 * geometry::SUBPIXELS_PER_PIXEL;

        assert_eq!(
            controls.crosshair.position(),
            SubpixelPoint {
                x: center_subpixels,
                y: center_subpixels,
            },
        );
    }

    #[test]
    fn receive_sends_no_input_after_a_snapshot_alone() {
        let mut session: ClientSession = create_joined_session();
        let state: GameState = create_server_state_with_own_member(create_alive_own_member());

        let effects: Vec<ClientEffect> = receive_message(&mut session, &create_snapshot_message(&state));

        assert_eq!(get_sent_messages(&effects), Vec::<MessageSerialIn>::new());
        assert_eq!(
            effects,
            vec![ClientEffect::OwnMemberStateChanged(
                OwnMemberStateKind::AliveParticipant
            )],
        );
    }

    #[test]
    fn receive_ignores_a_snapshot_of_another_game() {
        let mut session: ClientSession = create_joined_session();

        receive_message(
            &mut session,
            &MessageSerialOut::Snapshot(GameSnapshotSerialOut {
                game_id: GAME_ID.0 + 1,
                state: GameStateSerialOut::from(&create_server_state()),
            }),
        );

        assert_eq!(session.render_state(), None);
    }

    #[test]
    fn receive_samples_the_controls_into_an_input_after_each_applied_bundle() {
        let mut controls: PlayerControls = PlayerControls::new();
        let state: GameState = create_server_state_with_own_member(create_alive_own_member());
        let mut session: ClientSession = create_session_with_snapshot(&state, &mut controls);
        let cursor: SubpixelPoint = SubpixelPoint { x: 5000, y: 6000 };
        controls.crosshair.snap_to(cursor);
        press(&mut controls, GameKeyKind::ThirdAbility);

        let effects: Vec<ClientEffect> =
            receive_with_controls(&mut session, &create_bundle_message(Tick(1), Vec::new()), &mut controls);

        assert_eq!(
            get_sent_messages(&effects),
            vec![MessageSerialIn::Input(PlayerInputSerialIn {
                client_tick: 1,
                cursor: SubpixelPointSerial::from(&cursor),
                ability_presses: AbilityPressSet::THIRD.bits(),
                aim: None,
            })],
        );
        assert_eq!(controls.keyboard.pending_ability_presses(), AbilityPressSet::NONE);
    }

    #[test]
    fn receive_discards_the_presses_made_without_an_organism() {
        let mut controls: PlayerControls = PlayerControls::new();
        let state: GameState = create_server_state_with_own_member(test_fixture::create_participant(OWN_MEMBER_ID));
        let mut session: ClientSession = create_session_with_snapshot(&state, &mut controls);
        press(&mut controls, GameKeyKind::FirstAbility);

        let effects: Vec<ClientEffect> =
            receive_with_controls(&mut session, &create_bundle_message(Tick(1), Vec::new()), &mut controls);

        assert_eq!(get_sent_messages(&effects), Vec::<MessageSerialIn>::new());
        assert_eq!(controls.keyboard.pending_ability_presses(), AbilityPressSet::NONE);
    }

    #[test]
    fn receive_snaps_the_crosshair_to_the_own_spawn() {
        let mut controls: PlayerControls = PlayerControls::new();
        let mut session: ClientSession = create_session_with_snapshot(&create_server_state(), &mut controls);

        let effects: Vec<ClientEffect> = receive_with_controls(
            &mut session,
            &create_bundle_message(Tick(1), create_own_join_events(MemberRoleKindSerialOut::Participant)),
            &mut controls,
        );

        let own_cursor: WorldPoint =
            session.render_state().unwrap().members[&OWN_MEMBER_ID].organism.as_ref().unwrap().cursor;

        assert!(effects.contains(&ClientEffect::OwnOrganismSpawned { cursor: own_cursor }));
        assert!(effects.contains(&ClientEffect::OwnMemberStateChanged(
            OwnMemberStateKind::AliveParticipant
        )));
        assert_eq!(controls.crosshair.position(), own_cursor.to_subpixel_point());
        assert!(matches!(
            get_sent_messages(&effects)[..],
            [MessageSerialIn::Input(PlayerInputSerialIn { cursor, .. })]
                if cursor == SubpixelPointSerial::from(&own_cursor.to_subpixel_point())
        ));
    }

    #[test]
    fn receive_reports_an_own_spawn_rejected() {
        let mut controls: PlayerControls = PlayerControls::new();
        let mut state: GameState = create_server_state();
        state.settings.player_cap = 2;

        for member_id in [MemberId(0), OTHER_MEMBER_ID] {
            let position: WorldPoint = WorldPoint {
                x: 100 + 200 * i32::try_from(member_id.0).unwrap(),
                y: 100,
            };
            state.members.insert(
                member_id,
                test_fixture::create_participant_with_organism(member_id, position),
            );
        }

        let mut session: ClientSession = create_session_with_snapshot(&state, &mut controls);

        let effects: Vec<ClientEffect> = receive_with_controls(
            &mut session,
            &create_bundle_message(Tick(1), create_own_join_events(MemberRoleKindSerialOut::Participant)),
            &mut controls,
        );

        assert!(effects.contains(&ClientEffect::OwnSpawnRejected));
        assert_eq!(session.own_member_state(), Some(OwnMemberStateKind::DeadParticipant));
    }

    #[test]
    fn receive_reports_the_own_member_state_only_when_it_changes() {
        let mut controls: PlayerControls = PlayerControls::new();
        let mut session: ClientSession = create_session_with_snapshot(&create_server_state(), &mut controls);

        let joined_effects: Vec<ClientEffect> = receive_with_controls(
            &mut session,
            &create_bundle_message(Tick(1), create_own_join_events(MemberRoleKindSerialOut::Spectator)),
            &mut controls,
        );
        let later_effects: Vec<ClientEffect> =
            receive_with_controls(&mut session, &create_bundle_message(Tick(2), Vec::new()), &mut controls);

        assert_eq!(
            joined_effects,
            vec![
                ClientEffect::TickApplied { tick: Tick(1) },
                ClientEffect::OwnMemberStateChanged(OwnMemberStateKind::Spectator),
            ],
        );
        assert_eq!(later_effects, vec![ClientEffect::TickApplied { tick: Tick(2) }]);
    }

    #[test]
    fn receive_moves_the_crosshair_by_a_cursor_correction() {
        let mut controls: PlayerControls = PlayerControls::new();
        let state: GameState = create_server_state_with_own_member(create_alive_own_member());
        let mut session: ClientSession = create_session_with_snapshot(&state, &mut controls);
        controls.crosshair.snap_to(SubpixelPoint { x: 9000, y: 0 });
        receive_with_controls(&mut session, &create_bundle_message(Tick(1), Vec::new()), &mut controls);

        receive_with_controls(
            &mut session,
            &MessageSerialOut::CursorCorrected {
                client_tick: 1,
                cursor: SubpixelPointSerial { x: 8000, y: 0 },
            },
            &mut controls,
        );

        assert_eq!(controls.crosshair.position(), SubpixelPoint { x: 8000, y: 0 });
    }

    #[test]
    fn receive_discards_a_bundle_before_the_first_snapshot() {
        let mut session: ClientSession = create_joined_session();

        let effects: Vec<ClientEffect> = receive_message(&mut session, &create_bundle_message(Tick(1), Vec::new()));

        assert_eq!(effects, Vec::<ClientEffect>::new());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib play::client_session`

Expected: compile failure, first line ``error[E0425]: cannot find type `GameId` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/play/client_session.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::error::AppError;
use crate::game;
use crate::game::{GameId, GameState, GameSummary, InputBundle, SimulationEvent, StepError, Tick};
use crate::geometry::{SubpixelPoint, WorldPoint};
use crate::member::MemberId;
use crate::play::own_member_state;
use crate::play::{AbilityPressSample, FramePacer, OwnMemberStateKind, PlayerControls};
use crate::protocol;
use crate::protocol::{
    AimVectorSerial, CloseReasonKind, GameSnapshotSerialOut, GameSummarySerialOut, InputBundleSerialOut,
    MessageSerialIn, MessageSerialOut, PlayerInputSerialIn, RejectionKind, RequestKind, SubpixelPointSerial,
};
use crate::world::WorldBounds;

#[derive(Clone, Debug, PartialEq)]
pub enum ClientEffect {
    SendFrame(Vec<u8>),
    GameListReceived {
        game_summaries: Vec<GameSummary>,
        online_client_count: u32,
    },
    JoinAccepted {
        game_id: GameId,
        member_id: MemberId,
    },
    RequestRejected {
        request: RequestKind,
        reason: RejectionKind,
    },
    OwnMemberStateChanged(OwnMemberStateKind),
    OwnOrganismSpawned {
        cursor: WorldPoint,
    },
    OwnSpawnRejected,
    TickApplied {
        tick: Tick,
    },
    ResyncStarted,
    ResyncFinished,
    LeftGame,
    GameEnded,
    SynchronisationLost,
    /// `None` for an abnormal close, a failed open, or a close code no close reason uses.
    ConnectionLost(Option<CloseReasonKind>),
}

/// One per WebSocket connection.
#[derive(Clone, Debug, PartialEq)]
pub struct ClientSession {
    phase: SessionPhase,
    pacer: FramePacer,
}

impl ClientSession {
    pub fn new() -> ClientSession {
        ClientSession {
            phase: SessionPhase::Lobby,
            pacer: FramePacer::new(),
        }
    }

    pub fn receive(&mut self, frame: &[u8], controls: &mut PlayerControls, now_milliseconds: f64) -> Vec<ClientEffect> {
        let decoded: Result<MessageSerialOut, AppError> = protocol::decode_message_out(frame);

        match decoded {
            Ok(message_serial_out) => self.receive_message(message_serial_out, controls, now_milliseconds),
            Err(error) => {
                log::warn!("server frame dropped; [error={error}]");

                Vec::new()
            }
        }
    }

    pub fn send_request(&mut self, request: MessageSerialIn) -> Vec<ClientEffect> {
        vec![ClientEffect::SendFrame(protocol::encode_message_in(&request))]
    }

    pub fn connection_closed(&mut self, close_code: u16) -> Vec<ClientEffect> {
        self.phase = SessionPhase::Lobby;

        vec![ClientEffect::ConnectionLost(CloseReasonKind::from_close_code(
            close_code,
        ))]
    }

    /// `None` outside a game and until the first snapshot.
    pub fn render_state(&self) -> Option<&GameState> {
        self.get_game_session()?.confirmed_state.as_ref()
    }

    /// `None` outside a game and until the own member's `Joined` has been applied.
    pub fn own_member_state(&self) -> Option<OwnMemberStateKind> {
        self.get_game_session()?.own_member_state
    }

    pub fn game_id(&self) -> Option<GameId> {
        self.get_game_session().map(|game_session| game_session.game_id)
    }

    pub fn own_member_id(&self) -> Option<MemberId> {
        self.get_game_session().map(|game_session| game_session.member_id)
    }

    fn get_game_session(&self) -> Option<&GameSession> {
        match &self.phase {
            SessionPhase::InGame(game_session) => Some(game_session),
            SessionPhase::Lobby => None,
        }
    }

    fn receive_message(
        &mut self,
        message_serial_out: MessageSerialOut,
        controls: &mut PlayerControls,
        now_milliseconds: f64,
    ) -> Vec<ClientEffect> {
        match message_serial_out {
            MessageSerialOut::GameList {
                game_summaries,
                online_client_count,
            } => receive_game_list(game_summaries, online_client_count),
            MessageSerialOut::JoinAccepted { game_id, member_id } => {
                self.enter_game(GameId(game_id), MemberId(member_id))
            }
            MessageSerialOut::Snapshot(snapshot_serial_out) => {
                self.receive_snapshot(snapshot_serial_out, controls, now_milliseconds)
            }
            MessageSerialOut::InputBundle(bundle_serial_out) => {
                self.receive_bundle(bundle_serial_out, controls, now_milliseconds)
            }
            MessageSerialOut::StateChecksum { .. } => Vec::new(),
            MessageSerialOut::CursorCorrected { client_tick, cursor } => {
                self.receive_cursor_correction(Tick(client_tick), cursor, controls)
            }
            MessageSerialOut::RequestRejected { request, reason } => vec![ClientEffect::RequestRejected {
                request: RequestKind::from(request),
                reason: RejectionKind::from(reason),
            }],
            MessageSerialOut::LeftGame => self.leave_game(ClientEffect::LeftGame),
            MessageSerialOut::GameEnded => self.leave_game(ClientEffect::GameEnded),
        }
    }

    fn enter_game(&mut self, game_id: GameId, member_id: MemberId) -> Vec<ClientEffect> {
        self.phase = SessionPhase::InGame(GameSession::new(game_id, member_id));

        vec![ClientEffect::JoinAccepted { game_id, member_id }]
    }

    fn receive_snapshot(
        &mut self,
        snapshot_serial_out: GameSnapshotSerialOut,
        controls: &mut PlayerControls,
        now_milliseconds: f64,
    ) -> Vec<ClientEffect> {
        let SessionPhase::InGame(game_session) = &mut self.phase else {
            return Vec::new();
        };

        if GameId(snapshot_serial_out.game_id) != game_session.game_id {
            log::warn!(
                "snapshot of another game dropped; [game_id={}]",
                snapshot_serial_out.game_id
            );

            return Vec::new();
        }

        let converted: Result<GameState, AppError> = GameState::try_from(snapshot_serial_out.state);

        let state: GameState = match converted {
            Ok(state) => state,
            Err(error) => {
                log::warn!("snapshot dropped; [error={error}]");

                return Vec::new();
            }
        };

        self.pacer.record_tick_applied(now_milliseconds);

        game_session.apply_snapshot(state, controls)
    }

    fn receive_bundle(
        &mut self,
        bundle_serial_out: InputBundleSerialOut,
        controls: &mut PlayerControls,
        now_milliseconds: f64,
    ) -> Vec<ClientEffect> {
        let SessionPhase::InGame(game_session) = &mut self.phase else {
            return Vec::new();
        };

        let converted: Result<InputBundle, AppError> = InputBundle::try_from(bundle_serial_out);

        let bundle: InputBundle = match converted {
            Ok(bundle) => bundle,
            Err(error) => {
                log::warn!("bundle dropped; [error={error}]");

                return Vec::new();
            }
        };

        let applied: Result<Vec<ClientEffect>, StepError> = game_session.apply_bundle(&bundle, controls);

        match applied {
            Ok(effects) => {
                self.pacer.record_tick_applied(now_milliseconds);

                effects
            }
            Err(step_error) => {
                log::warn!(
                    "bundle could not be stepped; [tick={} error={step_error:?}]",
                    bundle.tick.0
                );

                Vec::new()
            }
        }
    }

    fn receive_cursor_correction(
        &mut self,
        client_tick: Tick,
        cursor_serial: SubpixelPointSerial,
        controls: &mut PlayerControls,
    ) -> Vec<ClientEffect> {
        if self.get_game_session().is_none() {
            return Vec::new();
        }

        let converted: Result<SubpixelPoint, AppError> = SubpixelPoint::try_from(cursor_serial);

        match converted {
            Ok(cursor) => controls.crosshair.apply_correction(client_tick, cursor),
            Err(error) => log::warn!("cursor correction dropped; [error={error}]"),
        }

        Vec::new()
    }

    /// `leave_effect` only when a game was left.
    fn leave_game(&mut self, leave_effect: ClientEffect) -> Vec<ClientEffect> {
        let previous_phase: SessionPhase = std::mem::replace(&mut self.phase, SessionPhase::Lobby);

        match previous_phase {
            SessionPhase::InGame(_) => vec![leave_effect],
            SessionPhase::Lobby => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum SessionPhase {
    Lobby,
    InGame(GameSession),
}

#[derive(Clone, Debug, PartialEq)]
struct GameSession {
    game_id: GameId,
    member_id: MemberId,
    /// `None` until the first snapshot.
    confirmed_state: Option<GameState>,
    own_member_state: Option<OwnMemberStateKind>,
}

impl GameSession {
    fn new(game_id: GameId, member_id: MemberId) -> GameSession {
        GameSession {
            game_id,
            member_id,
            confirmed_state: None,
            own_member_state: None,
        }
    }

    fn apply_snapshot(&mut self, state: GameState, controls: &mut PlayerControls) -> Vec<ClientEffect> {
        let is_first_snapshot: bool = self.confirmed_state.is_none();

        if is_first_snapshot {
            controls.crosshair.snap_to(get_world_center(&state));
        }

        self.confirmed_state = Some(state);

        self.report_own_member_state().into_iter().collect()
    }

    /// Bundles before the first snapshot are discarded.
    fn apply_bundle(
        &mut self,
        bundle: &InputBundle,
        controls: &mut PlayerControls,
    ) -> Result<Vec<ClientEffect>, StepError> {
        let Some(state) = self.confirmed_state.as_mut() else {
            return Ok(Vec::new());
        };

        let simulation_events: Vec<SimulationEvent> = game::step(state, bundle)?;
        let mut effects: Vec<ClientEffect> = vec![ClientEffect::TickApplied { tick: bundle.tick }];

        for simulation_event in &simulation_events {
            let own_event_effect: Option<ClientEffect> = self.handle_own_simulation_event(simulation_event, controls);
            effects.extend(own_event_effect);
        }

        effects.extend(self.report_own_member_state());
        effects.extend(self.sample_input(controls));

        Ok(effects)
    }

    fn handle_own_simulation_event(
        &self,
        simulation_event: &SimulationEvent,
        controls: &mut PlayerControls,
    ) -> Option<ClientEffect> {
        match simulation_event {
            SimulationEvent::OrganismSpawned { member_id, cursor } if *member_id == self.member_id => {
                controls.crosshair.snap_to(cursor.to_subpixel_point());

                Some(ClientEffect::OwnOrganismSpawned { cursor: *cursor })
            }
            SimulationEvent::SpawnRejected { member_id, .. } if *member_id == self.member_id => {
                Some(ClientEffect::OwnSpawnRejected)
            }
            SimulationEvent::MemberJoined { .. }
            | SimulationEvent::MemberLeft { .. }
            | SimulationEvent::OrganismSpawned { .. }
            | SimulationEvent::SpawnRejected { .. }
            | SimulationEvent::OrganismDied { .. }
            | SimulationEvent::RoundPhaseChanged { .. }
            | SimulationEvent::RoundWon { .. }
            | SimulationEvent::EffectApplied { .. } => None,
        }
    }

    fn report_own_member_state(&mut self) -> Option<ClientEffect> {
        let own_member_state: Option<OwnMemberStateKind> = self
            .confirmed_state
            .as_ref()
            .and_then(|state| own_member_state::get_own_member_state(state, self.member_id));

        if own_member_state == self.own_member_state {
            return None;
        }

        self.own_member_state = own_member_state;

        own_member_state.map(ClientEffect::OwnMemberStateChanged)
    }

    /// Only while the own member has an organism; presses made without one are discarded.
    fn sample_input(&self, controls: &mut PlayerControls) -> Option<ClientEffect> {
        let state: &GameState = self.confirmed_state.as_ref()?;
        let has_organism: bool = state.members.get(&self.member_id).is_some_and(|member| member.organism.is_some());

        if !has_organism {
            controls.keyboard.discard_ability_presses();

            return None;
        }

        let sample: AbilityPressSample = controls.keyboard.take_ability_presses();
        let cursor: SubpixelPoint = controls.crosshair.position();
        controls.crosshair.record_sent(state.tick, cursor);

        let input: MessageSerialIn = MessageSerialIn::Input(PlayerInputSerialIn {
            client_tick: state.tick.0,
            cursor: SubpixelPointSerial::from(&cursor),
            ability_presses: sample.ability_presses.bits(),
            aim: sample.aim.as_ref().map(AimVectorSerial::from),
        });

        Some(ClientEffect::SendFrame(protocol::encode_message_in(&input)))
    }
}

/// An unconvertible summary drops the whole list.
fn receive_game_list(
    game_summary_serial_outs: Vec<GameSummarySerialOut>,
    online_client_count: u32,
) -> Vec<ClientEffect> {
    let converted: Result<Vec<GameSummary>, AppError> =
        game_summary_serial_outs.into_iter().map(GameSummary::try_from).collect();

    match converted {
        Ok(game_summaries) => vec![ClientEffect::GameListReceived {
            game_summaries,
            online_client_count,
        }],
        Err(error) => {
            log::warn!("game list dropped; [error={error}]");

            Vec::new()
        }
    }
}

fn get_world_center(state: &GameState) -> SubpixelPoint {
    let bounds: &WorldBounds = &state.world.bounds;

    SubpixelPoint {
        x: bounds.left.0 + bounds.width.0 / 2,
        y: bounds.top.0 + bounds.height.0 / 2,
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib play::client_session`

Expected: `test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 381 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/play/mod.rs shared/src/play/client_session.rs
git commit -m "client session frames"
```

### Task 11: Client session synchronisation

**Files:**
- Create: `shared/src/play/active_scene.rs`
- Modify: `shared/src/play/client_session.rs`
- Modify: `shared/src/play/frame_pacing.rs`
- Modify: `shared/src/play/mod.rs`
- Test: inline `mod tests` in `shared/src/play/client_session.rs`

Checksums and resync (protocol.md, checksums and resync): a checksum mismatch after bundle t, a game frame which fails to decode or convert, a bundle which does not step, or a backlog over 43 ticks sends `RequestSnapshot` and discards bundles until the snapshot; the request is re-sent after 5 s; more than five resyncs within 60 s leave the game with `SynchronisationLost`. `advance_frame` moves the crosshair at the own member state's speed with the game focus, re-sends an overdue request and reports each applied tick once with the interpolation alpha; `FrameOutput` joins `frame_pacing.rs` and `InputFocusKind` starts `active_scene.rs` (its `ActiveScene` comes in Task 18).

- [ ] **Step 1: Write the failing tests**

In `shared/src/play/client_session.rs`, replace:

```rust
    use crate::world::WorldShapeKind;
```

with:

```rust
    use crate::protocol::{Pcg32SerialOut, PlayerTickInputSerialOut, WorldPointSerialOut};
    use crate::world::WorldShapeKind;
```

In `shared/src/play/client_session.rs`, replace:

```rust
    fn receive_with_controls(
        session: &mut ClientSession,
        message_serial_out: &MessageSerialOut,
        controls: &mut PlayerControls,
    ) -> Vec<ClientEffect> {
        let frame: Vec<u8> = protocol::encode_message_out(message_serial_out);

        session.receive(&frame, controls, NOW_MILLISECONDS)
    }
```

with:

```rust
    fn receive_with_controls(
        session: &mut ClientSession,
        message_serial_out: &MessageSerialOut,
        controls: &mut PlayerControls,
    ) -> Vec<ClientEffect> {
        receive_at(session, message_serial_out, controls, NOW_MILLISECONDS)
    }

    fn receive_at(
        session: &mut ClientSession,
        message_serial_out: &MessageSerialOut,
        controls: &mut PlayerControls,
        now_milliseconds: f64,
    ) -> Vec<ClientEffect> {
        let frame: Vec<u8> = protocol::encode_message_out(message_serial_out);

        session.receive(&frame, controls, now_milliseconds)
    }
```

In `shared/src/play/client_session.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    fn create_snapshot_request_effect(client_tick: Tick) -> ClientEffect {
        ClientEffect::SendFrame(protocol::encode_message_in(&MessageSerialIn::RequestSnapshot {
            client_tick: client_tick.0,
        }))
    }

    /// Snapshot of an alive own member at tick 0, then bundle 1, both at `NOW_MILLISECONDS`.
    fn create_playing_session(controls: &mut PlayerControls) -> (ClientSession, GameState) {
        let mut server_state: GameState = create_server_state_with_own_member(create_alive_own_member());
        let mut session: ClientSession = create_session_with_snapshot(&server_state, controls);
        let bundle_message: MessageSerialOut = create_bundle_message(Tick(1), Vec::new());
        receive_with_controls(&mut session, &bundle_message, controls);
        game::step(
            &mut server_state,
            &InputBundle {
                tick: Tick(1),
                member_events: Vec::new(),
                player_inputs: Vec::new(),
            },
        )
        .unwrap();

        (session, server_state)
    }

    fn create_mismatched_checksum_message(server_state: &GameState) -> MessageSerialOut {
        MessageSerialOut::StateChecksum {
            tick: server_state.tick.0,
            checksum: state_checksum::get_state_checksum(server_state) ^ 1,
        }
    }

    #[test]
    fn receive_accepts_a_matching_checksum() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut session, server_state): (ClientSession, GameState) = create_playing_session(&mut controls);

        let effects: Vec<ClientEffect> = receive_with_controls(
            &mut session,
            &MessageSerialOut::StateChecksum {
                tick: 1,
                checksum: state_checksum::get_state_checksum(&server_state),
            },
            &mut controls,
        );

        assert_eq!(effects, Vec::<ClientEffect>::new());
    }

    #[test]
    fn receive_resyncs_on_a_checksum_mismatch_and_discards_bundles_until_the_snapshot() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut session, mut server_state): (ClientSession, GameState) = create_playing_session(&mut controls);

        let mismatch_effects: Vec<ClientEffect> = receive_with_controls(
            &mut session,
            &create_mismatched_checksum_message(&server_state),
            &mut controls,
        );
        let discarded_effects: Vec<ClientEffect> =
            receive_with_controls(&mut session, &create_bundle_message(Tick(2), Vec::new()), &mut controls);

        assert_eq!(
            mismatch_effects,
            vec![create_snapshot_request_effect(Tick(1)), ClientEffect::ResyncStarted],
        );
        assert_eq!(discarded_effects, Vec::<ClientEffect>::new());
        assert_eq!(session.render_state().unwrap().tick, Tick(1));
        assert_eq!(session.message_text(), Some(String::from("Catching up")));

        server_state.tick = Tick(2);
        let snapshot_effects: Vec<ClientEffect> =
            receive_with_controls(&mut session, &create_snapshot_message(&server_state), &mut controls);
        let resumed_effects: Vec<ClientEffect> =
            receive_with_controls(&mut session, &create_bundle_message(Tick(3), Vec::new()), &mut controls);

        assert_eq!(snapshot_effects, vec![ClientEffect::ResyncFinished]);
        assert!(resumed_effects.contains(&ClientEffect::TickApplied { tick: Tick(3) }));
        assert_eq!(session.message_text(), None);
    }

    #[test]
    fn receive_resyncs_on_an_undecodable_game_frame() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut session, _): (ClientSession, GameState) = create_playing_session(&mut controls);

        let effects: Vec<ClientEffect> = session.receive(&[0xff, 0xff, 0xff], &mut controls, NOW_MILLISECONDS);

        assert_eq!(
            effects,
            vec![create_snapshot_request_effect(Tick(1)), ClientEffect::ResyncStarted],
        );
    }

    #[test]
    fn receive_resyncs_on_an_unconvertible_bundle() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut session, _): (ClientSession, GameState) = create_playing_session(&mut controls);

        let effects: Vec<ClientEffect> = receive_with_controls(
            &mut session,
            &MessageSerialOut::InputBundle(InputBundleSerialOut {
                tick: 2,
                member_events: Vec::new(),
                player_inputs: vec![PlayerTickInputSerialOut {
                    member_id: OWN_MEMBER_ID.0,
                    cursor: WorldPointSerialOut { x: 300, y: 200 },
                    ability_presses: 0xff,
                    aim: None,
                }],
            }),
            &mut controls,
        );

        assert_eq!(
            effects,
            vec![create_snapshot_request_effect(Tick(1)), ClientEffect::ResyncStarted],
        );
    }

    #[test]
    fn receive_resyncs_on_a_bundle_which_does_not_step() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut session, _): (ClientSession, GameState) = create_playing_session(&mut controls);

        let effects: Vec<ClientEffect> =
            receive_with_controls(&mut session, &create_bundle_message(Tick(5), Vec::new()), &mut controls);

        assert_eq!(
            effects,
            vec![create_snapshot_request_effect(Tick(1)), ClientEffect::ResyncStarted],
        );
    }

    #[test]
    fn receive_resyncs_on_an_unconvertible_snapshot() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut session, server_state): (ClientSession, GameState) = create_playing_session(&mut controls);
        let mut state_serial_out: GameStateSerialOut = GameStateSerialOut::from(&server_state);
        state_serial_out.rng = Pcg32SerialOut { state: 1, increment: 2 };

        let effects: Vec<ClientEffect> = receive_with_controls(
            &mut session,
            &MessageSerialOut::Snapshot(GameSnapshotSerialOut {
                game_id: GAME_ID.0,
                state: state_serial_out,
            }),
            &mut controls,
        );

        assert_eq!(
            effects,
            vec![create_snapshot_request_effect(Tick(1)), ClientEffect::ResyncStarted],
        );
    }

    #[test]
    fn receive_resyncs_when_the_applied_tick_falls_behind_by_more_than_the_backlog_limit() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut within_limit_session, server_state): (ClientSession, GameState) =
            create_playing_session(&mut controls);
        let (mut beyond_limit_session, _): (ClientSession, GameState) = create_playing_session(&mut controls);
        let checksum_message: MessageSerialOut = MessageSerialOut::StateChecksum {
            tick: 1,
            checksum: state_checksum::get_state_checksum(&server_state),
        };
        let backlog_limit_milliseconds: f64 =
            f64::from(BACKLOG_LIMIT_TICKS + 1) * frame_pacing::TICK_PERIOD_MILLISECONDS;

        let within_limit_effects: Vec<ClientEffect> = receive_at(
            &mut within_limit_session,
            &checksum_message,
            &mut controls,
            NOW_MILLISECONDS + backlog_limit_milliseconds,
        );
        let beyond_limit_effects: Vec<ClientEffect> = receive_at(
            &mut beyond_limit_session,
            &checksum_message,
            &mut controls,
            NOW_MILLISECONDS + backlog_limit_milliseconds + frame_pacing::TICK_PERIOD_MILLISECONDS,
        );

        assert_eq!(within_limit_effects, Vec::<ClientEffect>::new());
        assert_eq!(
            beyond_limit_effects,
            vec![create_snapshot_request_effect(Tick(1)), ClientEffect::ResyncStarted],
        );
    }

    #[test]
    fn advance_frame_sends_the_snapshot_request_again_after_the_response_timeout() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut session, server_state): (ClientSession, GameState) = create_playing_session(&mut controls);
        receive_with_controls(
            &mut session,
            &create_mismatched_checksum_message(&server_state),
            &mut controls,
        );

        let before_timeout_output: FrameOutput = session.advance_frame(
            NOW_MILLISECONDS + SNAPSHOT_RESPONSE_TIMEOUT_MILLISECONDS - 1.0,
            &mut controls,
            InputFocusKind::Game,
        );
        let at_timeout_output: FrameOutput = session.advance_frame(
            NOW_MILLISECONDS + SNAPSHOT_RESPONSE_TIMEOUT_MILLISECONDS,
            &mut controls,
            InputFocusKind::Game,
        );

        assert_eq!(before_timeout_output.effects, Vec::<ClientEffect>::new());
        assert_eq!(at_timeout_output.effects, vec![create_snapshot_request_effect(Tick(1))]);
    }

    #[test]
    fn receive_does_not_restart_a_resync_already_in_progress() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut session, server_state): (ClientSession, GameState) = create_playing_session(&mut controls);
        let mismatch_message: MessageSerialOut = create_mismatched_checksum_message(&server_state);
        receive_with_controls(&mut session, &mismatch_message, &mut controls);
        let during_resync_milliseconds: f64 = NOW_MILLISECONDS + 1000.0;

        let mismatch_effects: Vec<ClientEffect> =
            receive_at(&mut session, &mismatch_message, &mut controls, during_resync_milliseconds);
        let undecodable_effects: Vec<ClientEffect> =
            session.receive(&[0xff, 0xff, 0xff], &mut controls, during_resync_milliseconds);
        let at_timeout_output: FrameOutput = session.advance_frame(
            NOW_MILLISECONDS + SNAPSHOT_RESPONSE_TIMEOUT_MILLISECONDS,
            &mut controls,
            InputFocusKind::Game,
        );

        assert_eq!(mismatch_effects, Vec::<ClientEffect>::new());
        assert_eq!(undecodable_effects, Vec::<ClientEffect>::new());
        assert_eq!(at_timeout_output.effects, vec![create_snapshot_request_effect(Tick(1))]);
    }

    #[test]
    fn receive_ignores_a_checksum_for_a_tick_other_than_the_applied_tick() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut session, server_state): (ClientSession, GameState) = create_playing_session(&mut controls);

        let effects: Vec<ClientEffect> = receive_with_controls(
            &mut session,
            &MessageSerialOut::StateChecksum {
                tick: server_state.tick.plus(1).0,
                checksum: state_checksum::get_state_checksum(&server_state) ^ 1,
            },
            &mut controls,
        );

        assert_eq!(effects, Vec::<ClientEffect>::new());
    }

    #[test]
    fn receive_loses_synchronisation_after_more_than_the_resync_limit_within_the_window() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut session, server_state): (ClientSession, GameState) = create_playing_session(&mut controls);
        let mismatch_message: MessageSerialOut = create_mismatched_checksum_message(&server_state);

        for _ in 0..RESYNC_LIMIT {
            receive_with_controls(&mut session, &mismatch_message, &mut controls);
            receive_with_controls(&mut session, &create_snapshot_message(&server_state), &mut controls);
        }

        let lost_effects: Vec<ClientEffect> = receive_with_controls(&mut session, &mismatch_message, &mut controls);
        let left_effects: Vec<ClientEffect> =
            receive_with_controls(&mut session, &MessageSerialOut::LeftGame, &mut controls);

        assert_eq!(
            lost_effects,
            vec![
                ClientEffect::SendFrame(protocol::encode_message_in(&MessageSerialIn::LeaveGame)),
                ClientEffect::SynchronisationLost,
            ],
        );
        assert_eq!(left_effects, Vec::<ClientEffect>::new());
        assert_eq!(session.own_member_id(), None);
    }

    #[test]
    fn receive_counts_only_the_resyncs_within_the_window_against_the_limit() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut session, server_state): (ClientSession, GameState) = create_playing_session(&mut controls);
        let mismatch_message: MessageSerialOut = create_mismatched_checksum_message(&server_state);

        for _ in 0..RESYNC_LIMIT {
            receive_with_controls(&mut session, &mismatch_message, &mut controls);
            receive_with_controls(&mut session, &create_snapshot_message(&server_state), &mut controls);
        }

        let later_milliseconds: f64 = NOW_MILLISECONDS + RESYNC_LIMIT_WINDOW_MILLISECONDS;
        receive_at(
            &mut session,
            &create_snapshot_message(&server_state),
            &mut controls,
            later_milliseconds,
        );
        let effects: Vec<ClientEffect> = receive_at(&mut session, &mismatch_message, &mut controls, later_milliseconds);

        assert_eq!(
            effects,
            vec![create_snapshot_request_effect(Tick(1)), ClientEffect::ResyncStarted],
        );
    }

    #[test]
    fn advance_frame_moves_the_crosshair_only_with_the_game_focus() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut session, _): (ClientSession, GameState) = create_playing_session(&mut controls);
        let start: SubpixelPoint = controls.crosshair.position();
        press(&mut controls, GameKeyKind::MoveRight);

        session.advance_frame(NOW_MILLISECONDS, &mut controls, InputFocusKind::Game);
        session.advance_frame(NOW_MILLISECONDS + 100.0, &mut controls, InputFocusKind::Menu);

        assert_eq!(controls.crosshair.position(), start);

        session.advance_frame(NOW_MILLISECONDS + 200.0, &mut controls, InputFocusKind::Game);

        assert_eq!(
            controls.crosshair.position(),
            SubpixelPoint {
                x: start.x + 4352,
                y: start.y,
            },
        );
    }

    #[test]
    fn advance_frame_reports_each_applied_tick_once_with_the_interpolation_alpha() {
        let mut controls: PlayerControls = PlayerControls::new();
        let (mut session, _): (ClientSession, GameState) = create_playing_session(&mut controls);

        let first_output: FrameOutput =
            session.advance_frame(NOW_MILLISECONDS + 35.0, &mut controls, InputFocusKind::Game);
        let second_output: FrameOutput =
            session.advance_frame(NOW_MILLISECONDS + 70.0, &mut controls, InputFocusKind::Game);

        assert_eq!(first_output.tick_applied, Some(Tick(1)));
        assert_eq!(first_output.interpolation_alpha, 0.5);
        assert_eq!(second_output.tick_applied, None);
        assert_eq!(second_output.interpolation_alpha, 1.0);
    }

    #[test]
    fn message_text_gives_the_game_message_of_the_own_member() {
        let mut controls: PlayerControls = PlayerControls::new();
        let state: GameState = create_server_state_with_own_member(test_fixture::create_participant(OWN_MEMBER_ID));
        let session: ClientSession = create_session_with_snapshot(&state, &mut controls);

        assert_eq!(session.message_text(), Some(String::from("Press 'R' to Spawn")));
        assert_eq!(ClientSession::new().message_text(), None);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p shared --lib play::client_session`

Expected: compile failure, first line ``error[E0433]: cannot find module or crate `state_checksum` in this scope``.

- [ ] **Step 3: Implement**

Create `shared/src/play/active_scene.rs`:

```rust
/// Whether keyboard input steers the game or a menu over it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputFocusKind {
    Game,
    Menu,
}
```

Replace the whole contents of `shared/src/play/mod.rs`:

```rust
pub mod active_scene;
pub mod client_session;
pub mod crosshair;
pub mod frame_pacing;
pub mod game_key;
pub mod interpolation;
pub mod message_text;
pub mod own_member_state;
pub mod player_controls;

pub use active_scene::*;
pub use client_session::*;
pub use crosshair::*;
pub use frame_pacing::*;
pub use game_key::*;
pub use interpolation::*;
pub use message_text::*;
pub use own_member_state::*;
pub use player_controls::*;
```

In `shared/src/play/frame_pacing.rs`, replace:

```rust
use crate::game::tick;
```

with:

```rust
use crate::game::Tick;
use crate::game::tick;
use crate::play::ClientEffect;
```

In `shared/src/play/frame_pacing.rs`, replace:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct FramePacer {
```

with:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct FrameOutput {
    pub interpolation_alpha: f32,
    /// The newest tick applied since the previous frame.
    pub tick_applied: Option<Tick>,
    pub effects: Vec<ClientEffect>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FramePacer {
```

In `shared/src/play/client_session.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::error::AppError;
use crate::game;
use crate::game::{GameId, GameState, GameSummary, InputBundle, SimulationEvent, StepError, Tick};
use crate::geometry::{SubpixelPoint, WorldPoint};
use crate::member::MemberId;
use crate::play::{AbilityPressSample, FrameOutput, FramePacer, InputFocusKind, OwnMemberStateKind, PlayerControls};
use crate::play::{frame_pacing, message_text, own_member_state};
use crate::protocol;
use crate::protocol::state_checksum;
use crate::protocol::{
    AimVectorSerial, CloseReasonKind, GameSnapshotSerialOut, GameSummarySerialOut, InputBundleSerialOut,
    MessageSerialIn, MessageSerialOut, PlayerInputSerialIn, RejectionKind, RequestKind, SubpixelPointSerial,
};
use crate::world::WorldBounds;

pub const BACKLOG_LIMIT_TICKS: u32 = 43;
pub const SNAPSHOT_RESPONSE_TIMEOUT_MILLISECONDS: f64 = 5000.0;
/// More resyncs than this within the window lose the game.
pub const RESYNC_LIMIT: usize = 5;
pub const RESYNC_LIMIT_WINDOW_MILLISECONDS: f64 = 60_000.0;

#[derive(Clone, Debug, PartialEq)]
pub enum ClientEffect {
    SendFrame(Vec<u8>),
    GameListReceived {
        game_summaries: Vec<GameSummary>,
        online_client_count: u32,
    },
    JoinAccepted {
        game_id: GameId,
        member_id: MemberId,
    },
    RequestRejected {
        request: RequestKind,
        reason: RejectionKind,
    },
    OwnMemberStateChanged(OwnMemberStateKind),
    OwnOrganismSpawned {
        cursor: WorldPoint,
    },
    OwnSpawnRejected,
    TickApplied {
        tick: Tick,
    },
    ResyncStarted,
    ResyncFinished,
    LeftGame,
    GameEnded,
    SynchronisationLost,
    /// `None` for an abnormal close, a failed open, or a close code no close reason uses.
    ConnectionLost(Option<CloseReasonKind>),
}

/// One per WebSocket connection.
#[derive(Clone, Debug, PartialEq)]
pub struct ClientSession {
    phase: SessionPhase,
    pacer: FramePacer,
}

impl ClientSession {
    pub fn new() -> ClientSession {
        ClientSession {
            phase: SessionPhase::Lobby,
            pacer: FramePacer::new(),
        }
    }

    pub fn receive(&mut self, frame: &[u8], controls: &mut PlayerControls, now_milliseconds: f64) -> Vec<ClientEffect> {
        let decoded: Result<MessageSerialOut, AppError> = protocol::decode_message_out(frame);

        let mut effects: Vec<ClientEffect> = match decoded {
            Ok(message_serial_out) => self.receive_message(message_serial_out, controls, now_milliseconds),
            Err(error) => self.receive_undecodable_frame(&error, now_milliseconds),
        };

        effects.extend(self.check_backlog(now_milliseconds));

        effects
    }

    pub fn send_request(&mut self, request: MessageSerialIn) -> Vec<ClientEffect> {
        vec![ClientEffect::SendFrame(protocol::encode_message_in(&request))]
    }

    pub fn connection_closed(&mut self, close_code: u16) -> Vec<ClientEffect> {
        self.phase = SessionPhase::Lobby;

        vec![ClientEffect::ConnectionLost(CloseReasonKind::from_close_code(
            close_code,
        ))]
    }

    pub fn advance_frame(
        &mut self,
        now_milliseconds: f64,
        controls: &mut PlayerControls,
        input_focus: InputFocusKind,
    ) -> FrameOutput {
        let frame_duration_milliseconds: f64 = self.pacer.begin_frame(now_milliseconds);
        let speed_pixels_per_second: f64 =
            self.own_member_state().map_or(0.0, OwnMemberStateKind::crosshair_speed_pixels_per_second);

        if input_focus == InputFocusKind::Game {
            controls.crosshair.advance(&controls.keyboard, frame_duration_milliseconds, speed_pixels_per_second);
        }

        let effects: Vec<ClientEffect> = self.resend_overdue_snapshot_request(now_milliseconds);

        FrameOutput {
            interpolation_alpha: self.pacer.get_interpolation_alpha(now_milliseconds),
            tick_applied: self.take_unreported_tick(),
            effects,
        }
    }

    /// `None` outside a game and until the first snapshot.
    pub fn render_state(&self) -> Option<&GameState> {
        self.get_game_session()?.confirmed_state.as_ref()
    }

    /// `None` outside a game and until the own member's `Joined` has been applied.
    pub fn own_member_state(&self) -> Option<OwnMemberStateKind> {
        self.get_game_session()?.own_member_state
    }

    pub fn game_id(&self) -> Option<GameId> {
        self.get_game_session().map(|game_session| game_session.game_id)
    }

    pub fn own_member_id(&self) -> Option<MemberId> {
        self.get_game_session().map(|game_session| game_session.member_id)
    }

    /// The message box text; transient texts are the platform's.
    pub fn message_text(&self) -> Option<String> {
        let game_session: &GameSession = self.get_game_session()?;

        if game_session.is_resyncing() {
            return Some(String::from(message_text::CATCHING_UP_TEXT));
        }

        let state: &GameState = game_session.confirmed_state.as_ref()?;

        message_text::get_message_text(state, game_session.member_id)
    }

    fn get_game_session(&self) -> Option<&GameSession> {
        match &self.phase {
            SessionPhase::InGame(game_session) => Some(game_session),
            SessionPhase::Lobby | SessionPhase::Leaving => None,
        }
    }

    fn receive_message(
        &mut self,
        message_serial_out: MessageSerialOut,
        controls: &mut PlayerControls,
        now_milliseconds: f64,
    ) -> Vec<ClientEffect> {
        match message_serial_out {
            MessageSerialOut::GameList {
                game_summaries,
                online_client_count,
            } => receive_game_list(game_summaries, online_client_count),
            MessageSerialOut::JoinAccepted { game_id, member_id } => {
                self.enter_game(GameId(game_id), MemberId(member_id))
            }
            MessageSerialOut::Snapshot(snapshot_serial_out) => {
                self.receive_snapshot(snapshot_serial_out, controls, now_milliseconds)
            }
            MessageSerialOut::InputBundle(bundle_serial_out) => {
                self.receive_bundle(bundle_serial_out, controls, now_milliseconds)
            }
            MessageSerialOut::StateChecksum { tick, checksum } => {
                self.receive_checksum(Tick(tick), checksum, now_milliseconds)
            }
            MessageSerialOut::CursorCorrected { client_tick, cursor } => {
                self.receive_cursor_correction(Tick(client_tick), cursor, controls)
            }
            MessageSerialOut::RequestRejected { request, reason } => vec![ClientEffect::RequestRejected {
                request: RequestKind::from(request),
                reason: RejectionKind::from(reason),
            }],
            MessageSerialOut::LeftGame => self.leave_game(ClientEffect::LeftGame),
            MessageSerialOut::GameEnded => self.leave_game(ClientEffect::GameEnded),
        }
    }

    /// Outside a game such a frame is only logged.
    fn receive_undecodable_frame(&mut self, error: &AppError, now_milliseconds: f64) -> Vec<ClientEffect> {
        log::warn!("server frame undecodable; [error={error}]");

        self.start_resync(now_milliseconds)
    }

    fn enter_game(&mut self, game_id: GameId, member_id: MemberId) -> Vec<ClientEffect> {
        self.phase = SessionPhase::InGame(GameSession::new(game_id, member_id));

        vec![ClientEffect::JoinAccepted { game_id, member_id }]
    }

    fn receive_snapshot(
        &mut self,
        snapshot_serial_out: GameSnapshotSerialOut,
        controls: &mut PlayerControls,
        now_milliseconds: f64,
    ) -> Vec<ClientEffect> {
        let SessionPhase::InGame(game_session) = &mut self.phase else {
            return Vec::new();
        };

        if GameId(snapshot_serial_out.game_id) != game_session.game_id {
            log::warn!(
                "snapshot of another game dropped; [game_id={}]",
                snapshot_serial_out.game_id
            );

            return Vec::new();
        }

        let converted: Result<GameState, AppError> = GameState::try_from(snapshot_serial_out.state);

        let state: GameState = match converted {
            Ok(state) => state,
            Err(error) => {
                log::warn!("snapshot unconvertible; [error={error}]");

                return self.start_resync(now_milliseconds);
            }
        };

        self.pacer.record_tick_applied(now_milliseconds);

        game_session.apply_snapshot(state, controls, now_milliseconds)
    }

    fn receive_bundle(
        &mut self,
        bundle_serial_out: InputBundleSerialOut,
        controls: &mut PlayerControls,
        now_milliseconds: f64,
    ) -> Vec<ClientEffect> {
        let SessionPhase::InGame(game_session) = &mut self.phase else {
            return Vec::new();
        };

        if !game_session.is_synchronised() {
            return Vec::new();
        }

        let converted: Result<InputBundle, AppError> = InputBundle::try_from(bundle_serial_out);

        let bundle: InputBundle = match converted {
            Ok(bundle) => bundle,
            Err(error) => {
                log::warn!("bundle unconvertible; [error={error}]");

                return self.start_resync(now_milliseconds);
            }
        };

        let applied: Result<Vec<ClientEffect>, StepError> = game_session.apply_bundle(&bundle, controls);

        match applied {
            Ok(effects) => {
                self.pacer.record_tick_applied(now_milliseconds);

                effects
            }
            Err(step_error) => {
                log::warn!(
                    "bundle could not be stepped; [tick={} error={step_error:?}]",
                    bundle.tick.0
                );

                self.start_resync(now_milliseconds)
            }
        }
    }

    fn receive_checksum(&mut self, tick: Tick, checksum: u64, now_milliseconds: f64) -> Vec<ClientEffect> {
        let SessionPhase::InGame(game_session) = &self.phase else {
            return Vec::new();
        };

        let checksum_comparison: ChecksumComparisonKind = game_session.compare_checksum(tick, checksum);

        match checksum_comparison {
            ChecksumComparisonKind::Equal | ChecksumComparisonKind::NotComparable => Vec::new(),
            ChecksumComparisonKind::Different => {
                log::warn!("state checksum differs from the server's; [tick={}]", tick.0);

                self.start_resync(now_milliseconds)
            }
        }
    }

    fn receive_cursor_correction(
        &mut self,
        client_tick: Tick,
        cursor_serial: SubpixelPointSerial,
        controls: &mut PlayerControls,
    ) -> Vec<ClientEffect> {
        if self.get_game_session().is_none() {
            return Vec::new();
        }

        let converted: Result<SubpixelPoint, AppError> = SubpixelPoint::try_from(cursor_serial);

        match converted {
            Ok(cursor) => controls.crosshair.apply_correction(client_tick, cursor),
            Err(error) => log::warn!("cursor correction dropped; [error={error}]"),
        }

        Vec::new()
    }

    fn check_backlog(&mut self, now_milliseconds: f64) -> Vec<ClientEffect> {
        let SessionPhase::InGame(game_session) = &self.phase else {
            return Vec::new();
        };

        let Some(backlog_tick_count) = game_session.get_backlog_tick_count(now_milliseconds) else {
            return Vec::new();
        };

        if backlog_tick_count <= BACKLOG_LIMIT_TICKS {
            return Vec::new();
        }

        log::warn!("applied tick too far behind the server; [backlog_ticks={backlog_tick_count}]");

        self.start_resync(now_milliseconds)
    }

    /// Discards bundles until a snapshot arrives; past the resync limit, leaves the game instead.
    fn start_resync(&mut self, now_milliseconds: f64) -> Vec<ClientEffect> {
        let SessionPhase::InGame(game_session) = &mut self.phase else {
            return Vec::new();
        };

        let resync_start: ResyncStartKind = game_session.record_resync_start(now_milliseconds);

        match resync_start {
            ResyncStartKind::AlreadyResyncing => Vec::new(),
            ResyncStartKind::Started => vec![
                ClientEffect::SendFrame(game_session.encode_snapshot_request()),
                ClientEffect::ResyncStarted,
            ],
            ResyncStartKind::LimitExceeded => {
                log::warn!("synchronisation lost, leaving the game");
                self.phase = SessionPhase::Leaving;

                vec![
                    ClientEffect::SendFrame(protocol::encode_message_in(&MessageSerialIn::LeaveGame)),
                    ClientEffect::SynchronisationLost,
                ]
            }
        }
    }

    fn resend_overdue_snapshot_request(&mut self, now_milliseconds: f64) -> Vec<ClientEffect> {
        let SessionPhase::InGame(game_session) = &mut self.phase else {
            return Vec::new();
        };

        let overdue_snapshot_request: Option<Vec<u8>> = game_session.take_overdue_snapshot_request(now_milliseconds);

        match overdue_snapshot_request {
            Some(frame) => vec![ClientEffect::SendFrame(frame)],
            None => Vec::new(),
        }
    }

    fn take_unreported_tick(&mut self) -> Option<Tick> {
        let SessionPhase::InGame(game_session) = &mut self.phase else {
            return None;
        };

        game_session.take_unreported_tick()
    }

    /// `leave_effect` only when a game was left; the leave which ends a lost synchronisation is already reported.
    fn leave_game(&mut self, leave_effect: ClientEffect) -> Vec<ClientEffect> {
        let previous_phase: SessionPhase = std::mem::replace(&mut self.phase, SessionPhase::Lobby);

        match previous_phase {
            SessionPhase::InGame(_) => vec![leave_effect],
            SessionPhase::Lobby | SessionPhase::Leaving => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum SessionPhase {
    Lobby,
    InGame(GameSession),
    /// After a lost synchronisation, until the server confirms the leave.
    Leaving,
}

#[derive(Clone, Debug, PartialEq)]
struct GameSession {
    game_id: GameId,
    member_id: MemberId,
    /// `None` until the first snapshot.
    confirmed_state: Option<GameState>,
    synchronisation: SynchronisationKind,
    own_member_state: Option<OwnMemberStateKind>,
    /// The last tick `advance_frame` reported.
    reported_tick: Option<Tick>,
    resync_started_at_milliseconds: Vec<f64>,
}

impl GameSession {
    fn new(game_id: GameId, member_id: MemberId) -> GameSession {
        GameSession {
            game_id,
            member_id,
            confirmed_state: None,
            synchronisation: SynchronisationKind::AwaitingSnapshot {
                requested_at_milliseconds: None,
            },
            own_member_state: None,
            reported_tick: None,
            resync_started_at_milliseconds: Vec::new(),
        }
    }

    fn is_synchronised(&self) -> bool {
        matches!(self.synchronisation, SynchronisationKind::Synchronised { .. })
    }

    fn is_resyncing(&self) -> bool {
        matches!(
            self.synchronisation,
            SynchronisationKind::AwaitingSnapshot {
                requested_at_milliseconds: Some(_)
            }
        )
    }

    fn apply_snapshot(
        &mut self,
        state: GameState,
        controls: &mut PlayerControls,
        now_milliseconds: f64,
    ) -> Vec<ClientEffect> {
        let was_resyncing: bool = self.is_resyncing();
        let is_first_snapshot: bool = self.confirmed_state.is_none();

        if is_first_snapshot {
            controls.crosshair.snap_to(get_world_center(&state));
        }

        self.synchronisation = SynchronisationKind::Synchronised {
            snapshot_tick: state.tick,
            snapshot_received_at_milliseconds: now_milliseconds,
        };
        self.confirmed_state = Some(state);

        let mut effects: Vec<ClientEffect> = Vec::new();

        if was_resyncing {
            effects.push(ClientEffect::ResyncFinished);
        }

        effects.extend(self.report_own_member_state());

        effects
    }

    fn apply_bundle(
        &mut self,
        bundle: &InputBundle,
        controls: &mut PlayerControls,
    ) -> Result<Vec<ClientEffect>, StepError> {
        let Some(state) = self.confirmed_state.as_mut() else {
            return Ok(Vec::new());
        };

        let simulation_events: Vec<SimulationEvent> = game::step(state, bundle)?;
        let mut effects: Vec<ClientEffect> = vec![ClientEffect::TickApplied { tick: bundle.tick }];

        for simulation_event in &simulation_events {
            let own_event_effect: Option<ClientEffect> = self.handle_own_simulation_event(simulation_event, controls);
            effects.extend(own_event_effect);
        }

        effects.extend(self.report_own_member_state());
        effects.extend(self.sample_input(controls));

        Ok(effects)
    }

    fn handle_own_simulation_event(
        &self,
        simulation_event: &SimulationEvent,
        controls: &mut PlayerControls,
    ) -> Option<ClientEffect> {
        match simulation_event {
            SimulationEvent::OrganismSpawned { member_id, cursor } if *member_id == self.member_id => {
                controls.crosshair.snap_to(cursor.to_subpixel_point());

                Some(ClientEffect::OwnOrganismSpawned { cursor: *cursor })
            }
            SimulationEvent::SpawnRejected { member_id, .. } if *member_id == self.member_id => {
                Some(ClientEffect::OwnSpawnRejected)
            }
            SimulationEvent::MemberJoined { .. }
            | SimulationEvent::MemberLeft { .. }
            | SimulationEvent::OrganismSpawned { .. }
            | SimulationEvent::SpawnRejected { .. }
            | SimulationEvent::OrganismDied { .. }
            | SimulationEvent::RoundPhaseChanged { .. }
            | SimulationEvent::RoundWon { .. }
            | SimulationEvent::EffectApplied { .. } => None,
        }
    }

    fn report_own_member_state(&mut self) -> Option<ClientEffect> {
        let own_member_state: Option<OwnMemberStateKind> = self
            .confirmed_state
            .as_ref()
            .and_then(|state| own_member_state::get_own_member_state(state, self.member_id));

        if own_member_state == self.own_member_state {
            return None;
        }

        self.own_member_state = own_member_state;

        own_member_state.map(ClientEffect::OwnMemberStateChanged)
    }

    /// Only while the own member has an organism; presses made without one are discarded.
    fn sample_input(&self, controls: &mut PlayerControls) -> Option<ClientEffect> {
        let state: &GameState = self.confirmed_state.as_ref()?;
        let has_organism: bool = state.members.get(&self.member_id).is_some_and(|member| member.organism.is_some());

        if !has_organism {
            controls.keyboard.discard_ability_presses();

            return None;
        }

        let sample: AbilityPressSample = controls.keyboard.take_ability_presses();
        let cursor: SubpixelPoint = controls.crosshair.position();
        controls.crosshair.record_sent(state.tick, cursor);

        let input: MessageSerialIn = MessageSerialIn::Input(PlayerInputSerialIn {
            client_tick: state.tick.0,
            cursor: SubpixelPointSerial::from(&cursor),
            ability_presses: sample.ability_presses.bits(),
            aim: sample.aim.as_ref().map(AimVectorSerial::from),
        });

        Some(ClientEffect::SendFrame(protocol::encode_message_in(&input)))
    }

    /// Only a synchronised state at the checksum's tick is comparable.
    fn compare_checksum(&self, tick: Tick, checksum: u64) -> ChecksumComparisonKind {
        let Some(state) = self.confirmed_state.as_ref() else {
            return ChecksumComparisonKind::NotComparable;
        };

        if !self.is_synchronised() || state.tick != tick {
            return ChecksumComparisonKind::NotComparable;
        }

        if state_checksum::get_state_checksum(state) == checksum {
            return ChecksumComparisonKind::Equal;
        }

        ChecksumComparisonKind::Different
    }

    /// Ticks the applied state lags the server, which is at the snapshot tick plus the wall time since it arrived.
    fn get_backlog_tick_count(&self, now_milliseconds: f64) -> Option<u32> {
        let SynchronisationKind::Synchronised {
            snapshot_tick,
            snapshot_received_at_milliseconds,
        } = self.synchronisation
        else {
            return None;
        };

        let applied_tick: Tick = self.confirmed_state.as_ref()?.tick;
        let elapsed_tick_count: f64 =
            ((now_milliseconds - snapshot_received_at_milliseconds) / frame_pacing::TICK_PERIOD_MILLISECONDS).floor();
        let expected_tick: Tick = snapshot_tick.plus(elapsed_tick_count.max(0.0) as u32);

        Some(expected_tick.ticks_since(applied_tick))
    }

    fn record_resync_start(&mut self, now_milliseconds: f64) -> ResyncStartKind {
        if self.is_resyncing() {
            return ResyncStartKind::AlreadyResyncing;
        }

        self.resync_started_at_milliseconds.retain(|started_at_milliseconds| {
            now_milliseconds - started_at_milliseconds < RESYNC_LIMIT_WINDOW_MILLISECONDS
        });
        self.resync_started_at_milliseconds.push(now_milliseconds);

        if self.resync_started_at_milliseconds.len() > RESYNC_LIMIT {
            return ResyncStartKind::LimitExceeded;
        }

        self.synchronisation = SynchronisationKind::AwaitingSnapshot {
            requested_at_milliseconds: Some(now_milliseconds),
        };

        ResyncStartKind::Started
    }

    /// Restarts the response timer of a resync's snapshot request once it has run out.
    fn take_overdue_snapshot_request(&mut self, now_milliseconds: f64) -> Option<Vec<u8>> {
        let SynchronisationKind::AwaitingSnapshot {
            requested_at_milliseconds: Some(requested_at_milliseconds),
        } = self.synchronisation
        else {
            return None;
        };

        if now_milliseconds - requested_at_milliseconds < SNAPSHOT_RESPONSE_TIMEOUT_MILLISECONDS {
            return None;
        }

        self.synchronisation = SynchronisationKind::AwaitingSnapshot {
            requested_at_milliseconds: Some(now_milliseconds),
        };

        Some(self.encode_snapshot_request())
    }

    fn take_unreported_tick(&mut self) -> Option<Tick> {
        let applied_tick: Tick = self.confirmed_state.as_ref()?.tick;

        if self.reported_tick == Some(applied_tick) {
            return None;
        }

        self.reported_tick = Some(applied_tick);

        Some(applied_tick)
    }

    fn encode_snapshot_request(&self) -> Vec<u8> {
        let applied_tick: Tick = self.confirmed_state.as_ref().map_or(Tick(0), |state| state.tick);

        protocol::encode_message_in(&MessageSerialIn::RequestSnapshot {
            client_tick: applied_tick.0,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SynchronisationKind {
    Synchronised {
        snapshot_tick: Tick,
        snapshot_received_at_milliseconds: f64,
    },
    /// Bundles are discarded; `None` for the join snapshot, which the server sends unasked.
    AwaitingSnapshot { requested_at_milliseconds: Option<f64> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChecksumComparisonKind {
    Equal,
    Different,
    NotComparable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResyncStartKind {
    Started,
    AlreadyResyncing,
    LimitExceeded,
}

/// An unconvertible summary drops the whole list.
fn receive_game_list(
    game_summary_serial_outs: Vec<GameSummarySerialOut>,
    online_client_count: u32,
) -> Vec<ClientEffect> {
    let converted: Result<Vec<GameSummary>, AppError> =
        game_summary_serial_outs.into_iter().map(GameSummary::try_from).collect();

    match converted {
        Ok(game_summaries) => vec![ClientEffect::GameListReceived {
            game_summaries,
            online_client_count,
        }],
        Err(error) => {
            log::warn!("game list dropped; [error={error}]");

            Vec::new()
        }
    }
}

fn get_world_center(state: &GameState) -> SubpixelPoint {
    let bounds: &WorldBounds = &state.world.bounds;

    SubpixelPoint {
        x: bounds.left.0 + bounds.width.0 / 2,
        y: bounds.top.0 + bounds.height.0 / 2,
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared --lib -- play::client_session play::frame_pacing`

Expected: `test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 376 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/play/active_scene.rs shared/src/play/client_session.rs shared/src/play/frame_pacing.rs shared/src/play/mod.rs
git commit -m "client session synchronisation"
```

### Task 12: Two client sessions against the server

**Files:**
- Create: `server/tests/client_session.rs`
- Modify: `server/tests/helpers/test_server.rs`
- Test: `server/tests/client_session.rs`

The roadmap's done condition for this phase: a headless test drives two `ClientSession`s against the in-process server to identical checksums. Each `SessionDriver` plays as a browser would: server frames go to `receive`, `advance_frame` runs every 16 ms, every `SendFrame` goes to the socket. The creator holds right and presses its first ability; the joiner holds down. For every tick both sessions applied the checksums agree, every server `StateChecksum` matches the session's own, and neither resyncs. The behaviour under test is finished in Tasks 10 and 11; the test first fails only for the missing helper.

- [ ] **Step 1: Write the failing test**

Create `server/tests/client_session.rs`:

```rust
mod helpers;

use std::collections::BTreeMap;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time::{Instant, Interval};

use helpers::test_server;
use helpers::test_server::{TestClient, TestServer};
use shared::error::AppError;
use shared::game::{GameId, GameState, Tick};
use shared::play::{
    ClientEffect, ClientSession, FrameOutput, GameKeyKind, InputFocusKind, KeyRepeatKind, KeyTransitionKind,
    PlayerControls,
};
use shared::protocol;
use shared::protocol::state_checksum;
use shared::protocol::{MessageSerialIn, MessageSerialOut};

/// About one display frame.
const FRAME_PERIOD: Duration = Duration::from_millis(16);
const CREATOR_HEAD_START_TICK: Tick = Tick(3);
const FINAL_TICK: Tick = Tick(60);
const COMMON_TICK_MINIMUM: usize = 40;

enum DriverEventKind {
    FrameReceived(Vec<u8>),
    FrameDue,
}

/// A `ClientSession` connected to the test server, driven as a browser would drive it.
struct SessionDriver {
    client: TestClient,
    session: ClientSession,
    controls: PlayerControls,
    started_at: Instant,
    checksums_by_tick: BTreeMap<Tick, u64>,
    server_checksum_count: u32,
    resync_count: u32,
}

impl SessionDriver {
    async fn connect(test_server: &TestServer, held_keys: &[GameKeyKind]) -> SessionDriver {
        let mut controls: PlayerControls = PlayerControls::new();

        for held_key in held_keys {
            controls.keyboard.key_changed(*held_key, KeyTransitionKind::Pressed, KeyRepeatKind::Initial);
        }

        SessionDriver {
            client: TestClient::connect(test_server).await,
            session: ClientSession::new(),
            controls,
            started_at: Instant::now(),
            checksums_by_tick: BTreeMap::new(),
            server_checksum_count: 0,
            resync_count: 0,
        }
    }

    fn now_milliseconds(&self) -> f64 {
        self.started_at.elapsed().as_secs_f64() * 1000.0
    }

    fn applied_tick(&self) -> Option<Tick> {
        self.session.render_state().map(|state| state.tick)
    }

    async fn send_request(&mut self, request: MessageSerialIn) {
        let effects: Vec<ClientEffect> = self.session.send_request(request);
        self.handle_effects(effects).await;
    }

    async fn play_until_tick(&mut self, final_tick: Tick) {
        tokio::time::timeout(test_server::PLAY_TIMEOUT, self.play_frames_until_tick(final_tick))
            .await
            .expect("the session reaches the final tick within the play timeout");
    }

    async fn play_frames_until_tick(&mut self, final_tick: Tick) {
        let mut frame_interval: Interval = tokio::time::interval(FRAME_PERIOD);

        while self.applied_tick().is_none_or(|applied_tick| applied_tick < final_tick) {
            let driver_event: DriverEventKind = tokio::select! {
                frame = self.client.receive_binary_frame() => DriverEventKind::FrameReceived(frame),
                _ = frame_interval.tick() => DriverEventKind::FrameDue,
            };

            self.handle_driver_event(driver_event).await;
        }
    }

    async fn handle_driver_event(&mut self, driver_event: DriverEventKind) {
        let now_milliseconds: f64 = self.now_milliseconds();

        match driver_event {
            DriverEventKind::FrameReceived(frame) => {
                let effects: Vec<ClientEffect> = self.session.receive(&frame, &mut self.controls, now_milliseconds);
                self.handle_effects(effects).await;
                self.verify_server_checksum(&frame);
            }
            DriverEventKind::FrameDue => {
                let frame_output: FrameOutput =
                    self.session.advance_frame(now_milliseconds, &mut self.controls, InputFocusKind::Game);
                self.handle_effects(frame_output.effects).await;
            }
        }
    }

    async fn handle_effects(&mut self, effects: Vec<ClientEffect>) {
        for effect in effects {
            match effect {
                ClientEffect::SendFrame(frame) => self.client.send_raw(frame.into()).await,
                ClientEffect::TickApplied { tick } => self.record_checksum(tick),
                ClientEffect::ResyncStarted | ClientEffect::SynchronisationLost => self.resync_count += 1,
                _ => {}
            }
        }
    }

    fn record_checksum(&mut self, tick: Tick) {
        let state: &GameState = self.session.render_state().expect("an applied tick has a state");

        assert_eq!(state.tick, tick);
        self.checksums_by_tick.insert(tick, state_checksum::get_state_checksum(state));
    }

    /// The session compares the checksum itself; this also counts it.
    fn verify_server_checksum(&mut self, frame: &[u8]) {
        let decoded: Result<MessageSerialOut, AppError> = protocol::decode_message_out(frame);

        let Ok(MessageSerialOut::StateChecksum { tick, checksum }) = decoded else {
            return;
        };

        assert_eq!(self.checksums_by_tick.get(&Tick(tick)), Some(&checksum));
        self.server_checksum_count += 1;
    }
}

fn get_common_ticks(first_driver: &SessionDriver, second_driver: &SessionDriver) -> Vec<Tick> {
    first_driver
        .checksums_by_tick
        .keys()
        .filter(|tick| second_driver.checksums_by_tick.contains_key(tick))
        .copied()
        .collect()
}

fn spawn_play_until_tick(mut driver: SessionDriver, final_tick: Tick) -> JoinHandle<SessionDriver> {
    actix_web::rt::spawn(async move {
        driver.play_until_tick(final_tick).await;

        driver
    })
}

#[actix_web::test]
async fn two_client_sessions_compute_identical_checksums_for_one_game() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: SessionDriver =
        SessionDriver::connect(&test_server, &[GameKeyKind::MoveRight, GameKeyKind::FirstAbility]).await;
    creator.send_request(test_server::create_game_message("Arena", None, "Creator")).await;
    creator.play_until_tick(CREATOR_HEAD_START_TICK).await;

    let game_id: GameId = creator.session.game_id().expect("the creator is in its game");
    let mut joiner: SessionDriver = SessionDriver::connect(&test_server, &[GameKeyKind::MoveDown]).await;
    joiner.send_request(test_server::join_game_message(game_id.0, None, "Joiner")).await;

    let creator_task: JoinHandle<SessionDriver> = spawn_play_until_tick(creator, FINAL_TICK);
    let joiner_task: JoinHandle<SessionDriver> = spawn_play_until_tick(joiner, FINAL_TICK);
    let creator: SessionDriver = creator_task.await.expect("the creator plays");
    let joiner: SessionDriver = joiner_task.await.expect("the joiner plays");
    let common_ticks: Vec<Tick> = get_common_ticks(&creator, &joiner);

    assert!(common_ticks.len() >= COMMON_TICK_MINIMUM);

    for tick in &common_ticks {
        assert_eq!(creator.checksums_by_tick[tick], joiner.checksums_by_tick[tick]);
    }

    assert!(creator.server_checksum_count >= 1);
    assert!(joiner.server_checksum_count >= 1);
    assert_eq!((creator.resync_count, joiner.resync_count), (0, 0));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --test client_session`

Expected: compile failure, first line ``error[E0599]: no method named `receive_binary_frame` found for struct `TestClient` in the current scope``.

- [ ] **Step 3: Implement**

In `server/tests/helpers/test_server.rs`, replace:

```rust
    /// The next server message; fails on a close or after the receive timeout.
    pub async fn receive(&mut self) -> MessageSerialOut {
        let deadline: Instant = Instant::now() + RECEIVE_TIMEOUT;

        loop {
            let frame: Message = self.receive_frame(deadline).await;

            match frame {
                Message::Binary(frame) => {
                    return protocol::decode_message_out(&frame).expect("a server message decodes");
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                other_message => panic!("expected a server message, received {other_message:?}"),
            }
        }
    }
```

with:

```rust
    /// The next server message; fails on a close or after the receive timeout.
    pub async fn receive(&mut self) -> MessageSerialOut {
        let frame: Vec<u8> = self.receive_binary_frame().await;

        protocol::decode_message_out(&frame).expect("a server message decodes")
    }

    /// The next binary frame, undecoded; fails on a close or after the receive timeout.
    pub async fn receive_binary_frame(&mut self) -> Vec<u8> {
        let deadline: Instant = Instant::now() + RECEIVE_TIMEOUT;

        loop {
            let frame: Message = self.receive_frame(deadline).await;

            match frame {
                Message::Binary(frame) => return frame.to_vec(),
                Message::Ping(_) | Message::Pong(_) => continue,
                other_message => panic!("expected a binary frame, received {other_message:?}"),
            }
        }
    }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --test client_session`

Expected: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`. It takes about 5 s (60 server ticks).

- [ ] **Step 5: Run the other server integration tests, which share the helper**

Run: `cargo test -p server --test game_lifecycle --test protocol_errors 2>&1 | grep "^test result"`

Expected:

```text
test result: ok. 13 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 6: Commit**

```sh
git status
git add server/tests/helpers/test_server.rs server/tests/client_session.rs
git commit -m "two client sessions against the server"
```

### Task 13: Local game

**Files:**
- Create: `shared/src/play/local_game.rs`
- Modify: `shared/src/play/mod.rs`
- Modify: `shared/src/member/member.rs`
- Test: inline `mod tests` in `shared/src/play/local_game.rs`

What the title scene and the tutorial share (client-web.md, tutorial and title): `GameSettings` built directly (free for all, rectangle, the window less 25 px margins, the scene's player cap); the joiners join in tick 1 and are placed with `spawn::place_organism`, never through `SpawnRequested`; local ticks; placing again members without an organism; resize, which rescales the cursors and restarts each organism as one cell; the scene RNG on its own stream, and the colour and cell-coordinate draws. The palette order constants join `member.rs`.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/play/mod.rs`:

```rust
pub mod active_scene;
pub mod client_session;
pub mod crosshair;
pub mod frame_pacing;
pub mod game_key;
pub mod interpolation;
pub mod local_game;
pub mod message_text;
pub mod own_member_state;
pub mod player_controls;

pub use active_scene::*;
pub use client_session::*;
pub use crosshair::*;
pub use frame_pacing::*;
pub use game_key::*;
pub use interpolation::*;
pub use local_game::*;
pub use message_text::*;
pub use own_member_state::*;
pub use player_controls::*;
```

Create `shared/src/play/local_game.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::AbilityPressSet;
    use crate::game::{PlayerTickInput, Tick};
    use crate::geometry;
    use crate::member;
    use crate::member::{OrganismColorKind, SkinKind};

    const SEED: u64 = 0x10ca1;
    const WINDOW_SIZE: PixelSize = PixelSize {
        width_pixels: 850,
        height_pixels: 650,
    };

    fn create_local_joiner(cursor: WorldPoint) -> LocalJoiner {
        LocalJoiner {
            screen_name: String::from("Local"),
            appearance: Appearance {
                color: OrganismColorKind::Lake,
                skin: SkinKind::None,
            },
            cursor,
        }
    }

    fn create_local_game(cursors: &[WorldPoint]) -> LocalGame {
        let local_joiners: Vec<LocalJoiner> = cursors.iter().map(|cursor| create_local_joiner(*cursor)).collect();

        LocalGame::new(WINDOW_SIZE, 10, SEED, &local_joiners)
    }

    fn get_organism(local_game: &LocalGame, member_id: MemberId) -> &Organism {
        local_game.state().members[&member_id].organism.as_ref().unwrap()
    }

    #[test]
    fn get_world_size_takes_the_margins_off_the_window() {
        assert_eq!(
            get_world_size(WINDOW_SIZE),
            PixelSize {
                width_pixels: 800,
                height_pixels: 600,
            },
        );
        assert_eq!(
            get_world_size(PixelSize {
                width_pixels: 30,
                height_pixels: 0,
            }),
            PixelSize {
                width_pixels: 1,
                height_pixels: 1,
            },
        );
    }

    #[test]
    fn new_joins_every_local_joiner_in_the_first_tick_and_places_it_at_its_cursor() {
        let local_game: LocalGame = create_local_game(&[WorldPoint { x: 100, y: 120 }, WorldPoint { x: 500, y: 400 }]);
        let state: &GameState = local_game.state();

        assert_eq!(state.tick, Tick(1));
        assert_eq!(state.settings.mode, GameModeKind::FreeForAll);
        assert_eq!(state.settings.world_shape, WorldShapeKind::Rectangle);
        assert_eq!(
            (state.settings.world_width_pixels, state.settings.world_height_pixels),
            (800, 600),
        );
        assert_eq!(state.settings.player_cap, 10);
        assert_eq!(state.next_member_id, MemberId(2));
        assert_eq!(
            get_organism(&local_game, MemberId(1)).cursor,
            WorldPoint { x: 500, y: 400 }
        );
        assert_eq!(get_organism(&local_game, MemberId(1)).cells.count(), 1);
        assert_eq!(
            state.members[&MemberId(0)].loadout,
            Some(create_local_loadout(
                create_local_joiner(WorldPoint { x: 0, y: 0 }).appearance
            )),
        );
    }

    #[test]
    fn step_local_tick_steps_the_next_tick_with_the_inputs() {
        let mut local_game: LocalGame = create_local_game(&[WorldPoint { x: 100, y: 120 }]);

        local_game.step_local_tick(
            Vec::new(),
            vec![PlayerTickInput {
                member_id: MemberId(0),
                cursor: WorldPoint { x: 103, y: 120 },
                ability_presses: AbilityPressSet::NONE,
                aim: None,
            }],
        );

        assert_eq!(local_game.state().tick, Tick(2));
        assert_eq!(
            get_organism(&local_game, MemberId(0)).cursor,
            WorldPoint { x: 103, y: 120 }
        );
    }

    #[test]
    fn place_missing_organisms_places_only_members_without_an_organism() {
        let mut local_game: LocalGame =
            create_local_game(&[WorldPoint { x: 100, y: 120 }, WorldPoint { x: 500, y: 400 }]);
        local_game.state_mut().members.get_mut(&MemberId(1)).unwrap().organism = None;

        local_game.place_missing_organisms(&[
            (MemberId(0), WorldPoint { x: 1, y: 1 }),
            (MemberId(1), WorldPoint { x: 300, y: 310 }),
        ]);

        assert_eq!(
            get_organism(&local_game, MemberId(0)).cursor,
            WorldPoint { x: 100, y: 120 }
        );
        assert_eq!(
            get_organism(&local_game, MemberId(1)).cursor,
            WorldPoint { x: 300, y: 310 }
        );
    }

    #[test]
    fn resize_rescales_the_cursors_and_resets_the_organisms_to_one_cell() {
        let mut local_game: LocalGame = create_local_game(&[WorldPoint { x: 400, y: 300 }]);

        for _ in 0..20 {
            local_game.step_local_tick(Vec::new(), Vec::new());
        }

        assert!(get_organism(&local_game, MemberId(0)).cells.count() > 1);

        local_game.resize(PixelSize {
            width_pixels: 450,
            height_pixels: 350,
        });

        let organism: &Organism = get_organism(&local_game, MemberId(0));

        assert_eq!(organism.cursor, WorldPoint { x: 200, y: 150 });
        assert_eq!(organism.anchor, WorldPoint { x: 200, y: 150 });
        assert_eq!(organism.cells.count(), 1);
        assert_eq!(
            local_game.world_size(),
            PixelSize {
                width_pixels: 400,
                height_pixels: 300,
            },
        );
        assert_eq!(
            local_game.state().world.bounds.width.0,
            400 * geometry::SUBPIXELS_PER_PIXEL,
        );
    }

    #[test]
    fn rescale_point_keeps_the_proportion_of_the_world() {
        let old_size: PixelSize = PixelSize {
            width_pixels: 800,
            height_pixels: 600,
        };
        let new_size: PixelSize = PixelSize {
            width_pixels: 1200,
            height_pixels: 300,
        };

        assert_eq!(
            rescale_point(WorldPoint { x: 200, y: 450 }, old_size, new_size),
            WorldPoint { x: 300, y: 225 },
        );
    }

    #[test]
    fn draw_element_draws_from_the_elements_and_none_from_no_elements() {
        let mut rng: Pcg32 = create_scene_rng(SEED);

        let color: Option<OrganismColorKind> = draw_element(&mut rng, &member::ORGANISM_COLOR_ORDER);

        assert!(color.is_some_and(|color| member::ORGANISM_COLOR_ORDER.contains(&color)));
        assert_eq!(draw_element::<SkinKind>(&mut rng, &[]), None);
    }

    #[test]
    fn draw_color_excluding_never_draws_an_excluded_color() {
        let mut rng: Pcg32 = create_scene_rng(SEED);
        let excluded_colors: [OrganismColorKind; 2] = [OrganismColorKind::Sun, OrganismColorKind::Sky];

        for _ in 0..200 {
            assert!(!excluded_colors.contains(&draw_color_excluding(&mut rng, &excluded_colors)));
        }
    }

    #[test]
    fn draw_cell_coordinate_keeps_a_cell_clear_of_both_edges() {
        let mut rng: Pcg32 = create_scene_rng(SEED);
        let coordinates: Vec<i32> = (0..200).map(|_| draw_cell_coordinate(&mut rng, 100, 120)).collect();

        assert!(coordinates.iter().all(|coordinate| (107..114).contains(coordinate)));
        assert!(coordinates.contains(&107));
        assert!(coordinates.contains(&113));
        assert_eq!(draw_cell_coordinate(&mut rng, 0, 5), 7);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib play::local_game`

Expected: compile failure, first line ``error[E0425]: cannot find type `PixelSize` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/member/member.rs`, replace:

```rust
pub const TEAM_ORDER: [TeamKind; 4] = [TeamKind::Red, TeamKind::Blue, TeamKind::Green, TeamKind::Pink];
```

with:

```rust
pub const TEAM_ORDER: [TeamKind; 4] = [TeamKind::Red, TeamKind::Blue, TeamKind::Green, TeamKind::Pink];
pub const ORGANISM_COLOR_ORDER: [OrganismColorKind; 12] = [
    OrganismColorKind::Fire,
    OrganismColorKind::Camel,
    OrganismColorKind::Clay,
    OrganismColorKind::Sun,
    OrganismColorKind::Leaf,
    OrganismColorKind::Lime,
    OrganismColorKind::Sky,
    OrganismColorKind::Lake,
    OrganismColorKind::Ocean,
    OrganismColorKind::Royal,
    OrganismColorKind::Petal,
    OrganismColorKind::Hot,
];
pub const SKIN_ORDER: [SkinKind; 4] = [SkinKind::Grid, SkinKind::Circles, SkinKind::Ghost, SkinKind::None];
```

In `shared/src/play/local_game.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use crate::game;
use crate::game::{
    GameModeKind, GameSettings, GameState, InputBundle, MemberEvent, PlayerTickInput, SimulationEvent, StepError,
};
use crate::geometry;
use crate::geometry::{LatticeCoordinate, WorldPoint};
use crate::member;
use crate::member::{Appearance, MemberId, MemberRoleKind, OrganismColorKind};
use crate::organism::spawn;
use crate::organism::{CellOccupancy, Organism, PlacementError};
use crate::random::Pcg32;
use crate::world::{World, WorldBounds, WorldShapeKind};

/// The world is inset by this on every side of the window.
pub const LOCAL_WORLD_MARGIN_PIXELS: u32 = 25;
/// Separates the scene's own draws (colours, positions) from the game's.
pub const LOCAL_SCENE_RNG_STREAM: u64 = 0x5ce4_e5ce_4e5c_e4e5;
const LOCAL_GAME_TITLE: &str = "Local game";
const LOCAL_LEADERBOARD_LENGTH: u8 = 1;
const LOCAL_WORLD_MINIMUM_PIXELS: u32 = 1;
/// A cell centre this close to a world edge is outside the world.
const CELL_BORDER_PIXELS: i64 = geometry::CELL_WIDTH_PIXELS as i64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelSize {
    pub width_pixels: u32,
    pub height_pixels: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalJoiner {
    pub screen_name: String,
    pub appearance: Appearance,
    pub cursor: WorldPoint,
}

/// A game stepped by the client alone, with nobody to agree with, so its state may be edited between steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalGame {
    state: GameState,
}

impl LocalGame {
    /// The joiners join in tick 1 and are then placed at their cursors.
    pub fn new(window_size: PixelSize, player_cap: u8, seed: u64, local_joiners: &[LocalJoiner]) -> LocalGame {
        let settings: GameSettings = create_local_settings(get_world_size(window_size), player_cap);
        let mut local_game: LocalGame = LocalGame {
            state: GameState::new(settings, seed),
        };
        let joined_events: Vec<MemberEvent> = local_joiners
            .iter()
            .enumerate()
            .map(|(index, local_joiner)| create_joined_event(get_local_member_id(index), local_joiner))
            .collect();
        local_game.step_local_tick(joined_events, Vec::new());

        let cursors: Vec<(MemberId, WorldPoint)> = local_joiners
            .iter()
            .enumerate()
            .map(|(index, local_joiner)| (get_local_member_id(index), local_joiner.cursor))
            .collect();
        local_game.place_missing_organisms(&cursors);

        local_game
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut GameState {
        &mut self.state
    }

    pub fn world_size(&self) -> PixelSize {
        PixelSize {
            width_pixels: self.state.settings.world_width_pixels,
            height_pixels: self.state.settings.world_height_pixels,
        }
    }

    /// Local bundles always fit the state, so a step error is a defect, logged and skipped.
    pub fn step_local_tick(
        &mut self,
        member_events: Vec<MemberEvent>,
        player_inputs: Vec<PlayerTickInput>,
    ) -> Vec<SimulationEvent> {
        let bundle: InputBundle = InputBundle {
            tick: self.state.tick.next(),
            member_events,
            player_inputs,
        };

        let stepped: Result<Vec<SimulationEvent>, StepError> = game::step(&mut self.state, &bundle);

        match stepped {
            Ok(simulation_events) => simulation_events,
            Err(step_error) => {
                log::error!("local tick could not be stepped; [error={step_error:?}]");

                Vec::new()
            }
        }
    }

    /// Only members without an organism are placed.
    pub fn place_missing_organisms(&mut self, cursors: &[(MemberId, WorldPoint)]) {
        for (member_id, cursor) in cursors {
            let is_missing_organism: bool =
                self.state.members.get(member_id).is_some_and(|member| member.organism.is_none());

            if !is_missing_organism {
                continue;
            }

            let placed: Result<(), PlacementError> = spawn::place_organism(&mut self.state, *member_id, *cursor);

            if let Err(placement_error) = placed {
                log::error!("local organism could not be placed; [error={placement_error:?}]");
            }
        }
    }

    /// Every organism keeps its abilities and restarts as one cell at its rescaled cursor.
    pub fn resize(&mut self, window_size: PixelSize) {
        let old_size: PixelSize = self.world_size();
        let new_size: PixelSize = get_world_size(window_size);

        self.state.settings.world_width_pixels = new_size.width_pixels;
        self.state.settings.world_height_pixels = new_size.height_pixels;
        self.state.world = World::new(
            WorldShapeKind::Rectangle,
            WorldBounds::from_pixel_size(new_size.width_pixels, new_size.height_pixels),
        );

        for member in self.state.members.values_mut() {
            let Some(organism) = member.organism.as_mut() else {
                continue;
            };

            reset_to_one_cell(organism, rescale_point(organism.cursor, old_size, new_size));
        }
    }
}

pub fn get_world_size(window_size: PixelSize) -> PixelSize {
    PixelSize {
        width_pixels: get_inset_length(window_size.width_pixels),
        height_pixels: get_inset_length(window_size.height_pixels),
    }
}

/// The local games' loadout; only the appearance differs between members.
pub fn create_local_loadout(appearance: Appearance) -> Loadout {
    Loadout {
        appearance,
        first: FirstAbilityKind::Extend,
        second: SecondAbilityKind::Immortality,
        third: ThirdAbilityKind::Neutralize,
    }
}

pub fn create_joined_event(member_id: MemberId, local_joiner: &LocalJoiner) -> MemberEvent {
    MemberEvent::Joined {
        member_id,
        screen_name: local_joiner.screen_name.clone(),
        role: MemberRoleKind::Participant,
        loadout: Some(create_local_loadout(local_joiner.appearance)),
        team: None,
    }
}

pub fn rescale_point(point: WorldPoint, old_size: PixelSize, new_size: PixelSize) -> WorldPoint {
    WorldPoint {
        x: rescale_coordinate(point.x, old_size.width_pixels, new_size.width_pixels),
        y: rescale_coordinate(point.y, old_size.height_pixels, new_size.height_pixels),
    }
}

pub fn create_scene_rng(seed: u64) -> Pcg32 {
    Pcg32::from_seed_and_stream(seed, LOCAL_SCENE_RNG_STREAM)
}

/// `None` for no elements.
pub fn draw_element<T: Copy>(rng: &mut Pcg32, elements: &[T]) -> Option<T> {
    let element_count: u32 = u32::try_from(elements.len()).ok()?;
    let drawn_index: usize = usize::try_from(rng.below(element_count)).ok()?;

    elements.get(drawn_index).copied()
}

/// Uniform over the palette without `excluded_colors`.
pub fn draw_color_excluding(rng: &mut Pcg32, excluded_colors: &[OrganismColorKind]) -> OrganismColorKind {
    let colors: Vec<OrganismColorKind> =
        member::ORGANISM_COLOR_ORDER.into_iter().filter(|color| !excluded_colors.contains(color)).collect();

    draw_element(rng, &colors).unwrap_or(OrganismColorKind::Fire)
}

/// Uniform over the whole pixels between `start_pixels` and `end_pixels` at which a cell centre is clear of both.
pub fn draw_cell_coordinate(rng: &mut Pcg32, start_pixels: i64, end_pixels: i64) -> i32 {
    let minimum: i64 = start_pixels + CELL_BORDER_PIXELS + 1;
    let end: i64 = end_pixels - CELL_BORDER_PIXELS;
    let span: u32 = u32::try_from((end - minimum).max(0)).unwrap_or(0);

    i32::try_from(minimum + i64::from(rng.below(span))).unwrap_or(0)
}

fn create_local_settings(world_size: PixelSize, player_cap: u8) -> GameSettings {
    GameSettings {
        title: String::from(LOCAL_GAME_TITLE),
        mode: GameModeKind::FreeForAll,
        world_shape: WorldShapeKind::Rectangle,
        world_width_pixels: world_size.width_pixels,
        world_height_pixels: world_size.height_pixels,
        player_minimum: None,
        player_cap,
        team_count: None,
        leaderboard_length: LOCAL_LEADERBOARD_LENGTH,
    }
}

fn get_local_member_id(index: usize) -> MemberId {
    MemberId(u32::try_from(index).unwrap_or(u32::MAX))
}

fn get_inset_length(window_length_pixels: u32) -> u32 {
    window_length_pixels.saturating_sub(2 * LOCAL_WORLD_MARGIN_PIXELS).max(LOCAL_WORLD_MINIMUM_PIXELS)
}

fn reset_to_one_cell(organism: &mut Organism, cursor: WorldPoint) {
    organism.anchor = cursor;
    organism.cursor = cursor;
    organism.cells = CellOccupancy::with_cell(LatticeCoordinate { i: 0, j: 0 });
}

fn rescale_coordinate(coordinate: i32, old_length_pixels: u32, new_length_pixels: u32) -> i32 {
    let rescaled: i64 = i64::from(coordinate) * i64::from(new_length_pixels) / i64::from(old_length_pixels.max(1));

    i32::try_from(rescaled).unwrap_or(coordinate)
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib play::local_game`

Expected: `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 414 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/play/mod.rs shared/src/play/local_game.rs shared/src/member/member.rs
git commit -m "local game"
```

### Task 14: Title scene

**Files:**
- Create: `shared/src/title/mod.rs`
- Create: `shared/src/title/title_scene.rs`
- Modify: `shared/src/lib.rs`
- Test: inline `mod tests` in `shared/src/title/title_scene.rs`

Ten dummy organisms with random palette colours and skins (None included), at most `floor(10/4) + 1 = 3` per quadrant, fixed cursors, in the inset world; resize rescales the cursors and resets the organisms to one cell (client-web.md, tutorial and title; the original's `Title.js`).

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/lib.rs`:

```rust
//! Everything the server and the clients must agree on, plus platform-independent client logic.

pub mod ability;
pub mod error;
pub mod game;
pub mod geometry;
pub mod member;
pub mod organism;
pub mod play;
pub mod protocol;
pub mod random;
pub mod replay;
pub mod round;
pub mod settings;
pub mod title;
pub mod world;
```

Create `shared/src/title/mod.rs`:

```rust
pub mod title_scene;

pub use title_scene::*;
```

Create `shared/src/title/title_scene.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Tick;
    use crate::organism::Organism;

    const SEED: u64 = 0x7171e;
    const WINDOW_SIZE: PixelSize = PixelSize {
        width_pixels: 1050,
        height_pixels: 650,
    };

    fn get_dummy_organisms(title_scene: &TitleScene) -> Vec<&Organism> {
        title_scene.state().members.values().filter_map(|member| member.organism.as_ref()).collect()
    }

    fn get_quadrant_index(cursor: WorldPoint, world_size: PixelSize) -> usize {
        let is_right: bool = cursor.x >= i32::try_from(world_size.width_pixels / 2).unwrap();
        let is_bottom: bool = cursor.y >= i32::try_from(world_size.height_pixels / 2).unwrap();

        QUADRANT_OFFSETS
            .iter()
            .position(|offsets| *offsets == (i64::from(is_right), i64::from(is_bottom)))
            .unwrap()
    }

    #[test]
    fn new_places_every_dummy_inside_the_world_with_at_most_the_quadrant_limit_per_quadrant() {
        let title_scene: TitleScene = TitleScene::new(WINDOW_SIZE, SEED);
        let world_size: PixelSize = local_game::get_world_size(WINDOW_SIZE);
        let dummy_organisms: Vec<&Organism> = get_dummy_organisms(&title_scene);
        let mut quadrant_counts: [u32; 4] = [0; 4];

        for organism in &dummy_organisms {
            quadrant_counts[get_quadrant_index(organism.cursor, world_size)] += 1;

            assert!(title_scene.state().world.contains_cell(organism.cursor));
        }

        assert_eq!(dummy_organisms.len(), TITLE_DUMMY_COUNT);
        assert!(quadrant_counts.iter().all(|quadrant_count| *quadrant_count <= TITLE_QUADRANT_LIMIT));
    }

    #[test]
    fn new_draws_the_same_scene_for_the_same_seed() {
        assert_eq!(TitleScene::new(WINDOW_SIZE, SEED), TitleScene::new(WINDOW_SIZE, SEED));
        assert_ne!(
            TitleScene::new(WINDOW_SIZE, SEED).state(),
            TitleScene::new(WINDOW_SIZE, SEED + 1).state(),
        );
    }

    #[test]
    fn advance_frame_steps_one_local_tick_per_tick_period() {
        let mut title_scene: TitleScene = TitleScene::new(WINDOW_SIZE, SEED);

        let first_output: FrameOutput = title_scene.advance_frame(1000.0);
        let second_output: FrameOutput = title_scene.advance_frame(1070.0);
        let third_output: FrameOutput = title_scene.advance_frame(1210.0);

        assert_eq!(first_output.tick_applied, None);
        assert_eq!(second_output.tick_applied, Some(Tick(2)));
        assert_eq!(third_output.tick_applied, Some(Tick(4)));
        assert_eq!(third_output.interpolation_alpha, 0.0);
        assert_eq!(third_output.effects, Vec::new());
    }

    #[test]
    fn advance_frame_places_a_dead_dummy_again_at_its_cursor() {
        let mut title_scene: TitleScene = TitleScene::new(WINDOW_SIZE, SEED);
        let cursor: WorldPoint = title_scene.state().members[&MemberId(3)].organism.as_ref().unwrap().cursor;
        title_scene.local_game.state_mut().members.get_mut(&MemberId(3)).unwrap().organism = None;

        title_scene.advance_frame(0.0);
        title_scene.advance_frame(70.0);

        let organism: &Organism = title_scene.state().members[&MemberId(3)].organism.as_ref().unwrap();

        assert_eq!(organism.cursor, cursor);
        assert_eq!(organism.cells.count(), 1);
    }

    #[test]
    fn resize_rescales_the_dummy_cursors_and_resets_the_dummies_to_one_cell() {
        let mut title_scene: TitleScene = TitleScene::new(WINDOW_SIZE, SEED);
        let cursor: WorldPoint = title_scene.state().members[&MemberId(0)].organism.as_ref().unwrap().cursor;

        title_scene.resize(PixelSize {
            width_pixels: 2050,
            height_pixels: 1250,
        });

        let organism: &Organism = title_scene.state().members[&MemberId(0)].organism.as_ref().unwrap();

        assert_eq!(
            organism.cursor,
            WorldPoint {
                x: cursor.x * 2,
                y: cursor.y * 2,
            },
        );
        assert_eq!(organism.cells.count(), 1);
        assert_eq!(title_scene.dummy_cursors[0], (MemberId(0), organism.cursor));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib title::title_scene`

Expected: compile failure, first line ``error[E0425]: cannot find type `PixelSize` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/title/title_scene.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::game::{GameState, Tick};
use crate::geometry::WorldPoint;
use crate::member;
use crate::member::{Appearance, MemberId, SkinKind};
use crate::play::local_game;
use crate::play::{FrameOutput, FramePacer, LocalGame, LocalJoiner, PixelSize};
use crate::random::Pcg32;

pub const TITLE_DUMMY_COUNT: usize = 10;
pub const TITLE_QUADRANT_LIMIT: u32 = TITLE_DUMMY_COUNT as u32 / 4 + 1;
const TITLE_PLAYER_CAP: u8 = 10;
/// In half-world units: top left, top right, bottom right, bottom left.
const QUADRANT_OFFSETS: [(i64, i64); 4] = [(0, 0), (1, 0), (1, 1), (0, 1)];

/// Dummy organisms growing in a local game, never steered.
#[derive(Clone, Debug, PartialEq)]
pub struct TitleScene {
    local_game: LocalGame,
    pacer: FramePacer,
    /// A dummy's cursor never moves; a dummy without an organism is placed again there.
    dummy_cursors: Vec<(MemberId, WorldPoint)>,
}

impl TitleScene {
    pub fn new(window_size: PixelSize, seed: u64) -> TitleScene {
        let mut scene_rng: Pcg32 = local_game::create_scene_rng(seed);
        let dummies: Vec<LocalJoiner> = create_dummies(&mut scene_rng, local_game::get_world_size(window_size));
        let local_game: LocalGame = LocalGame::new(window_size, TITLE_PLAYER_CAP, seed, &dummies);
        let dummy_cursors: Vec<(MemberId, WorldPoint)> = local_game
            .state()
            .members
            .values()
            .filter_map(|member| member.organism.as_ref().map(|organism| (member.member_id, organism.cursor)))
            .collect();

        TitleScene {
            local_game,
            pacer: FramePacer::new(),
            dummy_cursors,
        }
    }

    pub fn state(&self) -> &GameState {
        self.local_game.state()
    }

    pub fn advance_frame(&mut self, now_milliseconds: f64) -> FrameOutput {
        let frame_duration_milliseconds: f64 = self.pacer.begin_frame(now_milliseconds);
        let local_tick_count: u32 = self.pacer.take_local_tick_count(frame_duration_milliseconds);
        let mut tick_applied: Option<Tick> = None;

        for _ in 0..local_tick_count {
            self.local_game.step_local_tick(Vec::new(), Vec::new());
            self.local_game.place_missing_organisms(&self.dummy_cursors);
            tick_applied = Some(self.local_game.state().tick);
        }

        if tick_applied.is_some() {
            self.pacer.record_tick_applied(now_milliseconds);
        }

        FrameOutput {
            interpolation_alpha: self.pacer.get_interpolation_alpha(now_milliseconds),
            tick_applied,
            effects: Vec::new(),
        }
    }

    pub fn resize(&mut self, window_size: PixelSize) {
        let old_size: PixelSize = self.local_game.world_size();
        self.local_game.resize(window_size);
        let new_size: PixelSize = self.local_game.world_size();

        for (_, cursor) in self.dummy_cursors.iter_mut() {
            *cursor = local_game::rescale_point(*cursor, old_size, new_size);
        }
    }
}

fn create_dummies(scene_rng: &mut Pcg32, world_size: PixelSize) -> Vec<LocalJoiner> {
    let mut quadrant_counts: [u32; 4] = [0; 4];

    (0..TITLE_DUMMY_COUNT)
        .map(|index| LocalJoiner {
            screen_name: format!("Dummy {index}"),
            appearance: draw_appearance(scene_rng),
            cursor: draw_cursor(scene_rng, world_size, &mut quadrant_counts),
        })
        .collect()
}

/// Any palette colour and any skin, `None` included.
fn draw_appearance(scene_rng: &mut Pcg32) -> Appearance {
    Appearance {
        color: local_game::draw_color_excluding(scene_rng, &[]),
        skin: local_game::draw_element(scene_rng, &member::SKIN_ORDER).unwrap_or(SkinKind::None),
    }
}

/// Uniform in a quadrant drawn among those still below the quadrant limit.
fn draw_cursor(scene_rng: &mut Pcg32, world_size: PixelSize, quadrant_counts: &mut [u32; 4]) -> WorldPoint {
    let open_quadrant_indices: Vec<usize> = (0..QUADRANT_OFFSETS.len())
        .filter(|quadrant_index| quadrant_counts[*quadrant_index] < TITLE_QUADRANT_LIMIT)
        .collect();
    let quadrant_index: usize = local_game::draw_element(scene_rng, &open_quadrant_indices).unwrap_or(0);
    quadrant_counts[quadrant_index] += 1;

    let (x_offset, y_offset): (i64, i64) = QUADRANT_OFFSETS[quadrant_index];

    WorldPoint {
        x: draw_quadrant_coordinate(scene_rng, world_size.width_pixels, x_offset),
        y: draw_quadrant_coordinate(scene_rng, world_size.height_pixels, y_offset),
    }
}

fn draw_quadrant_coordinate(scene_rng: &mut Pcg32, world_length_pixels: u32, half_offset: i64) -> i32 {
    let half_length: i64 = i64::from(world_length_pixels / 2);

    local_game::draw_cell_coordinate(scene_rng, half_length * half_offset, half_length * (half_offset + 1))
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib title::title_scene`

Expected: `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 423 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/lib.rs shared/src/title/mod.rs shared/src/title/title_scene.rs
git commit -m "title scene"
```

### Task 15: Tutorial bot

**Files:**
- Create: `shared/src/tutorial/mod.rs`
- Create: `shared/src/tutorial/tutorial_bot.rs`
- Modify: `shared/src/lib.rs`
- Test: inline `mod tests` in `shared/src/tutorial/tutorial_bot.rs`

The bot of the compress task: a uniform random point inside the world at least `TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS = 80` from the player's cursor, a random palette colour except Sun and Lime, skin None (client-web.md, tutorial and title). `mod.rs` declares only `tutorial_bot` until Task 16 adds `tutorial`.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/lib.rs`:

```rust
//! Everything the server and the clients must agree on, plus platform-independent client logic.

pub mod ability;
pub mod error;
pub mod game;
pub mod geometry;
pub mod member;
pub mod organism;
pub mod play;
pub mod protocol;
pub mod random;
pub mod replay;
pub mod round;
pub mod settings;
pub mod title;
pub mod tutorial;
pub mod world;
```

Create `shared/src/tutorial/mod.rs`:

```rust
pub mod tutorial_bot;

pub use tutorial_bot::*;
```

Create `shared/src/tutorial/tutorial_bot.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const WORLD_SIZE: PixelSize = PixelSize {
        width_pixels: 400,
        height_pixels: 300,
    };
    const PLAYER_CURSOR: WorldPoint = WorldPoint { x: 200, y: 150 };

    #[test]
    fn create_tutorial_bot_joiner_places_the_bot_inside_the_world_away_from_the_player() {
        for seed in 0..100 {
            let mut scene_rng: Pcg32 = local_game::create_scene_rng(seed);

            let bot_joiner: LocalJoiner = create_tutorial_bot_joiner(&mut scene_rng, WORLD_SIZE, PLAYER_CURSOR);
            let distance_squared: i64 = bot_joiner.cursor.distance_squared(PLAYER_CURSOR);

            assert!(distance_squared >= TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS * TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS);
            assert!((7..394).contains(&bot_joiner.cursor.x));
            assert!((7..294).contains(&bot_joiner.cursor.y));
        }
    }

    #[test]
    fn create_tutorial_bot_joiner_draws_neither_an_excluded_color_nor_a_skin() {
        for seed in 0..100 {
            let mut scene_rng: Pcg32 = local_game::create_scene_rng(seed);

            let appearance: Appearance =
                create_tutorial_bot_joiner(&mut scene_rng, WORLD_SIZE, PLAYER_CURSOR).appearance;

            assert!(!TUTORIAL_BOT_EXCLUDED_COLORS.contains(&appearance.color));
            assert_eq!(appearance.skin, SkinKind::None);
        }
    }

    #[test]
    fn create_tutorial_bot_joiner_takes_the_last_candidate_in_a_world_too_small_for_the_distance() {
        let mut scene_rng: Pcg32 = local_game::create_scene_rng(0);
        let small_world_size: PixelSize = PixelSize {
            width_pixels: 60,
            height_pixels: 60,
        };

        let bot_joiner: LocalJoiner =
            create_tutorial_bot_joiner(&mut scene_rng, small_world_size, WorldPoint { x: 30, y: 30 });

        assert!((7..54).contains(&bot_joiner.cursor.x));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib tutorial::tutorial_bot`

Expected: compile failure, first line ``error[E0425]: cannot find type `PixelSize` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/tutorial/tutorial_bot.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::geometry::WorldPoint;
use crate::member::{Appearance, MemberId, OrganismColorKind, SkinKind};
use crate::play::local_game;
use crate::play::{LocalJoiner, PixelSize};
use crate::random::Pcg32;

/// The original's default range plus 30.
pub const TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS: i64 = 80;
pub const TUTORIAL_BOT_EXCLUDED_COLORS: [OrganismColorKind; 2] = [OrganismColorKind::Sun, OrganismColorKind::Lime];
/// A world too small for the minimum distance takes the last candidate.
const BOT_POSITION_ATTEMPT_LIMIT: u32 = 64;
const TUTORIAL_BOT_SCREEN_NAME: &str = "Bot";

/// The target of the compress, freeze, toxin and spore tasks; its cursor never moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TutorialBot {
    pub member_id: MemberId,
    pub cursor: WorldPoint,
}

pub fn create_tutorial_bot_joiner(
    scene_rng: &mut Pcg32,
    world_size: PixelSize,
    player_cursor: WorldPoint,
) -> LocalJoiner {
    LocalJoiner {
        screen_name: String::from(TUTORIAL_BOT_SCREEN_NAME),
        appearance: Appearance {
            color: local_game::draw_color_excluding(scene_rng, &TUTORIAL_BOT_EXCLUDED_COLORS),
            skin: SkinKind::None,
        },
        cursor: draw_bot_cursor(scene_rng, world_size, player_cursor),
    }
}

fn draw_bot_cursor(scene_rng: &mut Pcg32, world_size: PixelSize, player_cursor: WorldPoint) -> WorldPoint {
    let minimum_distance_squared: i64 = TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS * TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS;
    let mut candidate: WorldPoint = draw_world_point(scene_rng, world_size);

    for _ in 1..BOT_POSITION_ATTEMPT_LIMIT {
        if candidate.distance_squared(player_cursor) >= minimum_distance_squared {
            return candidate;
        }

        candidate = draw_world_point(scene_rng, world_size);
    }

    candidate
}

fn draw_world_point(scene_rng: &mut Pcg32, world_size: PixelSize) -> WorldPoint {
    WorldPoint {
        x: local_game::draw_cell_coordinate(scene_rng, 0, i64::from(world_size.width_pixels)),
        y: local_game::draw_cell_coordinate(scene_rng, 0, i64::from(world_size.height_pixels)),
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib tutorial::tutorial_bot`

Expected: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 428 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/lib.rs shared/src/tutorial/mod.rs shared/src/tutorial/tutorial_bot.rs
git commit -m "tutorial bot"
```

### Task 16: Tutorial tasks

**Files:**
- Create: `shared/src/tutorial/tutorial.rs`
- Modify: `shared/src/tutorial/mod.rs`
- Test: inline `mod tests` in `shared/src/tutorial/tutorial.rs`

The task sequence of the original's `Tutorial.js` and the texts of its `Message.js`, verbatim: move, fullscreen (skipped in fullscreen; F11 or 3.5 s), survive (4.5 s), extend, immortality, neutralize, shoot (10 s), compress, freeze, toxin, spore, done. One ability is enabled per task, as the original's `activated` toggles; the loadout switches to Compress and Freeze from the shoot task on and to Toxin from the toxin task on; the HUD is hidden during move and survive. The filter `tutorial::tutorial::` selects this file's tests and not the bot's.

- [ ] **Step 1: Write the failing test**

Replace the whole contents of `shared/src/tutorial/mod.rs`:

```rust
pub mod tutorial;
pub mod tutorial_bot;

pub use tutorial::*;
pub use tutorial_bot::*;
```

Create `shared/src/tutorial/tutorial.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const TASK_ORDER: [TutorialTaskKind; 12] = [
        TutorialTaskKind::Move,
        TutorialTaskKind::Fullscreen,
        TutorialTaskKind::Survive,
        TutorialTaskKind::Extend,
        TutorialTaskKind::Immortality,
        TutorialTaskKind::Neutralize,
        TutorialTaskKind::Shoot,
        TutorialTaskKind::Compress,
        TutorialTaskKind::Freeze,
        TutorialTaskKind::Toxin,
        TutorialTaskKind::Spore,
        TutorialTaskKind::Done,
    ];

    #[test]
    fn next_follows_the_task_order_and_stays_done() {
        let next_tasks: Vec<TutorialTaskKind> =
            TASK_ORDER.iter().map(|task| task.next(DisplayModeKind::Windowed)).collect();

        assert_eq!(next_tasks[..11], TASK_ORDER[1..]);
        assert_eq!(next_tasks[11], TutorialTaskKind::Done);
    }

    #[test]
    fn next_skips_the_fullscreen_task_in_fullscreen() {
        assert_eq!(
            TutorialTaskKind::Move.next(DisplayModeKind::Fullscreen),
            TutorialTaskKind::Survive,
        );
    }

    #[test]
    fn enabled_abilities_enables_the_abilities_of_the_task_only() {
        let enabled_abilities: Vec<AbilityPressSet> = TASK_ORDER.iter().map(|task| task.enabled_abilities()).collect();

        assert_eq!(
            enabled_abilities,
            vec![
                AbilityPressSet::NONE,
                AbilityPressSet::NONE,
                AbilityPressSet::NONE,
                AbilityPressSet::FIRST,
                AbilityPressSet::SECOND,
                AbilityPressSet::THIRD,
                AbilityPressSet::FIRST.with(AbilityPressSet::SECOND),
                AbilityPressSet::FIRST,
                AbilityPressSet::SECOND,
                AbilityPressSet::THIRD,
                AbilityPressSet::FOURTH,
                AbilityPressSet::NONE,
            ],
        );
    }

    #[test]
    fn ability_kinds_switch_to_the_shot_abilities_and_then_to_toxin() {
        let extend_task: TutorialTaskKind = TutorialTaskKind::Extend;
        let shoot_task: TutorialTaskKind = TutorialTaskKind::Shoot;
        let toxin_task: TutorialTaskKind = TutorialTaskKind::Toxin;

        assert_eq!(
            (
                extend_task.first_ability(),
                extend_task.second_ability(),
                extend_task.third_ability()
            ),
            (
                FirstAbilityKind::Extend,
                SecondAbilityKind::Immortality,
                ThirdAbilityKind::Neutralize
            ),
        );
        assert_eq!(
            (
                shoot_task.first_ability(),
                shoot_task.second_ability(),
                shoot_task.third_ability()
            ),
            (
                FirstAbilityKind::Compress,
                SecondAbilityKind::Freeze,
                ThirdAbilityKind::Neutralize
            ),
        );
        assert_eq!(
            (
                toxin_task.first_ability(),
                toxin_task.second_ability(),
                toxin_task.third_ability()
            ),
            (
                FirstAbilityKind::Compress,
                SecondAbilityKind::Freeze,
                ThirdAbilityKind::Toxin
            ),
        );
    }

    #[test]
    fn is_hud_visible_hides_the_hud_during_the_move_and_survive_tasks() {
        let hidden_tasks: Vec<TutorialTaskKind> =
            TASK_ORDER.into_iter().filter(|task| !task.is_hud_visible()).collect();

        assert_eq!(hidden_tasks, vec![TutorialTaskKind::Move, TutorialTaskKind::Survive]);
    }

    #[test]
    fn message_text_gives_the_original_task_texts() {
        assert_eq!(
            TutorialTaskKind::Move.message_text(TutorialSteppingKind::Running),
            "Use W-A-S-D (Recommended) or the arrow keys to move",
        );
        assert_eq!(
            TutorialTaskKind::Compress.message_text(TutorialSteppingKind::Running),
            "On hit, COMPRESS shrinks the size of the targeted enemy\nCOMPRESS the bot to progress",
        );
        assert_eq!(
            TutorialTaskKind::Spore.message_text(TutorialSteppingKind::PausedForSecretion),
            "Reactivate the ability to cause all spores to secrete an acid, killing enemy cells",
        );
        assert_eq!(
            TutorialTaskKind::Done.message_text(TutorialSteppingKind::Running),
            "Now that you're ready, press ESC to return to the menu",
        );
    }

    #[test]
    fn timed_task_ticks_times_the_fullscreen_survive_and_shoot_tasks() {
        let timed_tasks: Vec<(TutorialTaskKind, u32)> = TASK_ORDER
            .into_iter()
            .filter_map(|task| task.timed_task_ticks().map(|task_ticks| (task, task_ticks)))
            .collect();

        assert_eq!(
            timed_tasks,
            vec![
                (TutorialTaskKind::Fullscreen, 50),
                (TutorialTaskKind::Survive, 64),
                (TutorialTaskKind::Shoot, 143),
            ],
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib tutorial::tutorial::`

Expected: compile failure, first line ``error[E0425]: cannot find type `TutorialTaskKind` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/tutorial/tutorial.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::ability::{AbilityPressSet, FirstAbilityKind, SecondAbilityKind, ThirdAbilityKind};
use crate::game::tick;

pub const FULLSCREEN_TASK_TICKS: u32 = tick::get_ticks_from_milliseconds(3500);
pub const SURVIVE_TASK_TICKS: u32 = tick::get_ticks_from_milliseconds(4500);
pub const SHOOT_TASK_TICKS: u32 = tick::get_ticks_from_milliseconds(10_000);

/// In task order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TutorialTaskKind {
    Move,
    Fullscreen,
    Survive,
    Extend,
    Immortality,
    Neutralize,
    Shoot,
    Compress,
    Freeze,
    Toxin,
    Spore,
    Done,
}

impl TutorialTaskKind {
    pub fn next(self, display_mode: DisplayModeKind) -> TutorialTaskKind {
        match self {
            TutorialTaskKind::Move => match display_mode {
                DisplayModeKind::Windowed => TutorialTaskKind::Fullscreen,
                DisplayModeKind::Fullscreen => TutorialTaskKind::Survive,
            },
            TutorialTaskKind::Fullscreen => TutorialTaskKind::Survive,
            TutorialTaskKind::Survive => TutorialTaskKind::Extend,
            TutorialTaskKind::Extend => TutorialTaskKind::Immortality,
            TutorialTaskKind::Immortality => TutorialTaskKind::Neutralize,
            TutorialTaskKind::Neutralize => TutorialTaskKind::Shoot,
            TutorialTaskKind::Shoot => TutorialTaskKind::Compress,
            TutorialTaskKind::Compress => TutorialTaskKind::Freeze,
            TutorialTaskKind::Freeze => TutorialTaskKind::Toxin,
            TutorialTaskKind::Toxin => TutorialTaskKind::Spore,
            TutorialTaskKind::Spore | TutorialTaskKind::Done => TutorialTaskKind::Done,
        }
    }

    /// `None` for a task which ends on a condition.
    pub fn timed_task_ticks(self) -> Option<u32> {
        match self {
            TutorialTaskKind::Fullscreen => Some(FULLSCREEN_TASK_TICKS),
            TutorialTaskKind::Survive => Some(SURVIVE_TASK_TICKS),
            TutorialTaskKind::Shoot => Some(SHOOT_TASK_TICKS),
            TutorialTaskKind::Move
            | TutorialTaskKind::Extend
            | TutorialTaskKind::Immortality
            | TutorialTaskKind::Neutralize
            | TutorialTaskKind::Compress
            | TutorialTaskKind::Freeze
            | TutorialTaskKind::Toxin
            | TutorialTaskKind::Spore
            | TutorialTaskKind::Done => None,
        }
    }

    /// The original's per-task `activated` toggles.
    pub fn enabled_abilities(self) -> AbilityPressSet {
        match self {
            TutorialTaskKind::Move
            | TutorialTaskKind::Fullscreen
            | TutorialTaskKind::Survive
            | TutorialTaskKind::Done => AbilityPressSet::NONE,
            TutorialTaskKind::Extend | TutorialTaskKind::Compress => AbilityPressSet::FIRST,
            TutorialTaskKind::Immortality | TutorialTaskKind::Freeze => AbilityPressSet::SECOND,
            TutorialTaskKind::Neutralize | TutorialTaskKind::Toxin => AbilityPressSet::THIRD,
            TutorialTaskKind::Shoot => AbilityPressSet::FIRST.with(AbilityPressSet::SECOND),
            TutorialTaskKind::Spore => AbilityPressSet::FOURTH,
        }
    }

    pub fn first_ability(self) -> FirstAbilityKind {
        if self >= TutorialTaskKind::Shoot {
            return FirstAbilityKind::Compress;
        }

        FirstAbilityKind::Extend
    }

    pub fn second_ability(self) -> SecondAbilityKind {
        if self >= TutorialTaskKind::Shoot {
            return SecondAbilityKind::Freeze;
        }

        SecondAbilityKind::Immortality
    }

    pub fn third_ability(self) -> ThirdAbilityKind {
        if self >= TutorialTaskKind::Toxin {
            return ThirdAbilityKind::Toxin;
        }

        ThirdAbilityKind::Neutralize
    }

    pub fn is_hud_visible(self) -> bool {
        !matches!(self, TutorialTaskKind::Move | TutorialTaskKind::Survive)
    }

    /// Verbatim from the original's `Message.js`.
    pub fn message_text(self, stepping: TutorialSteppingKind) -> &'static str {
        match (self, stepping) {
            (TutorialTaskKind::Move, _) => "Use W-A-S-D (Recommended) or the arrow keys to move",
            (TutorialTaskKind::Fullscreen, _) => "Press F11 to enter fullscreen mode (Recommended)",
            (TutorialTaskKind::Survive, _) => "If the crosshair is too far from the organism, it will die",
            (TutorialTaskKind::Extend, _) => "Use the EXTEND ability to increase the organism's size",
            (TutorialTaskKind::Immortality, _) => "The IMMORTALITY ability will stop the natural atrophe of cells",
            (TutorialTaskKind::Neutralize, _) => "NEUTRALIZE will create a bubble of safety from enemy attacks",
            (TutorialTaskKind::Shoot, _) => {
                "To COMPRESS or FREEZE an enemy, press the ability key to launch a spore in the direction of the cursor\nThen press it again to activate the ability"
            }
            (TutorialTaskKind::Compress, _) => {
                "On hit, COMPRESS shrinks the size of the targeted enemy\nCOMPRESS the bot to progress"
            }
            (TutorialTaskKind::Freeze, _) => {
                "On hit, FREEZE halts all natural processes within the enemy organism\nFREEZE the bot to progress"
            }
            (TutorialTaskKind::Toxin, _) => {
                "TOXIN creates a localized bubble in which only you can survive\nDamage the bot to progress"
            }
            (TutorialTaskKind::Spore, TutorialSteppingKind::Running) => {
                "Use SPORE to jettison outer cells in all directions (Space Bar)\nDamage the bot to progress"
            }
            (TutorialTaskKind::Spore, TutorialSteppingKind::PausedForSecretion) => {
                "Reactivate the ability to cause all spores to secrete an acid, killing enemy cells"
            }
            (TutorialTaskKind::Done, _) => "Now that you're ready, press ESC to return to the menu",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayModeKind {
    Windowed,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TutorialSteppingKind {
    Running,
    /// Halfway through a spore flight in the spore task, until the spores are made to secrete.
    PausedForSecretion,
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib tutorial::tutorial::`

Expected: `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 431 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/tutorial/mod.rs shared/src/tutorial/tutorial.rs
git commit -m "tutorial tasks"
```

### Task 17: Tutorial

**Files:**
- Modify: `shared/src/tutorial/tutorial.rs`
- Test: inline `mod tests` in `shared/src/tutorial/tutorial.rs`

The tutorial as a local game (client-web.md, tutorial and title): the player at the world centre under the crosshair, a random palette colour except Sun and Sky, skin None; the crosshair at 42.5 px/s with the game focus; presses masked by the task before each local bundle; task timers as tick deadlines owned by `Tutorial`, so leaving drops them; the loadout written into the player's `Member` between steps; the bot joined for the compress task; every organism with zero cells placed again at its own cursor before the next step; the spore task pausing at half the flight until the spores are made to secrete. The harness steps one frame per tick period; tests of the conditions met by an applied shot or a hit call `advance_task` with the event.

- [ ] **Step 1: Write the failing tests**

In `shared/src/tutorial/tutorial.rs`, replace:

```rust
mod tests {
    use super::*;
```

with:

```rust
mod tests {
    use super::*;
    use crate::geometry::SubpixelPoint;
    use crate::play::frame_pacing;
    use crate::play::{KeyRepeatKind, KeyTransitionKind};

    const SEED: u64 = 0x7070;
    const WINDOW_SIZE: PixelSize = PixelSize {
        width_pixels: 850,
        height_pixels: 650,
    };
    const WORLD_CENTER: WorldPoint = WorldPoint { x: 400, y: 300 };

    struct TutorialHarness {
        tutorial: Tutorial,
        controls: PlayerControls,
        now_milliseconds: f64,
    }

    impl TutorialHarness {
        fn new(display_mode: DisplayModeKind) -> TutorialHarness {
            let mut controls: PlayerControls = PlayerControls::new();
            let mut tutorial: Tutorial = Tutorial::new(WINDOW_SIZE, SEED, display_mode, &mut controls);
            tutorial.advance_frame(0.0, &mut controls, InputFocusKind::Game);

            TutorialHarness {
                tutorial,
                controls,
                now_milliseconds: 0.0,
            }
        }

        /// One frame per tick period.
        fn run_ticks(&mut self, tick_count: u32) {
            for _ in 0..tick_count {
                self.now_milliseconds += frame_pacing::TICK_PERIOD_MILLISECONDS;
                self.tutorial.advance_frame(self.now_milliseconds, &mut self.controls, InputFocusKind::Game);
            }
        }

        fn change_key(&mut self, key: GameKeyKind, transition: KeyTransitionKind) {
            self.controls.keyboard.key_changed(key, transition, KeyRepeatKind::Initial);
        }

        fn press(&mut self, key: GameKeyKind) {
            self.change_key(key, KeyTransitionKind::Pressed);
            self.change_key(key, KeyTransitionKind::Released);
        }

        fn start_task(&mut self, task: TutorialTaskKind) {
            self.tutorial.start_task(task);
        }

        fn player_organism(&self) -> &Organism {
            self.tutorial.get_player_organism().unwrap()
        }

        fn bot(&self) -> TutorialBot {
            self.tutorial.bot.unwrap()
        }

        fn bot_organism_mut(&mut self) -> &mut Organism {
            let bot_member_id: MemberId = self.bot().member_id;

            self.tutorial
                .local_game
                .state_mut()
                .members
                .get_mut(&bot_member_id)
                .unwrap()
                .organism
                .as_mut()
                .unwrap()
        }

        /// The task's condition, met in the tick just stepped.
        fn meet_condition(&mut self, simulation_events: &[SimulationEvent]) {
            self.tutorial.advance_task(&AbilityPressSample::NONE, simulation_events);
        }
    }
```

In `shared/src/tutorial/tutorial.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn new_places_the_player_at_the_world_center_under_the_crosshair() {
        let harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        let player_organism: &Organism = harness.player_organism();

        assert_eq!(player_organism.cursor, WORLD_CENTER);
        assert_eq!(harness.controls.crosshair.position(), WORLD_CENTER.to_subpixel_point());
        assert_eq!(harness.tutorial.task(), TutorialTaskKind::Move);
        assert_eq!(harness.tutorial.state().settings.player_cap, 2);

        let appearance: Appearance = harness.tutorial.state().members[&TUTORIAL_PLAYER_ID].loadout.unwrap().appearance;

        assert!(!TUTORIAL_PLAYER_EXCLUDED_COLORS.contains(&appearance.color));
        assert_eq!(appearance.skin, SkinKind::None);
    }

    #[test]
    fn advance_frame_ends_the_move_task_once_a_movement_key_is_held() {
        let mut windowed_harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        let mut fullscreen_harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Fullscreen);

        windowed_harness.run_ticks(3);

        assert_eq!(windowed_harness.tutorial.task(), TutorialTaskKind::Move);

        for harness in [&mut windowed_harness, &mut fullscreen_harness] {
            harness.change_key(GameKeyKind::MoveUp, KeyTransitionKind::Pressed);
            harness.run_ticks(1);
        }

        assert_eq!(windowed_harness.tutorial.task(), TutorialTaskKind::Fullscreen);
        assert_eq!(fullscreen_harness.tutorial.task(), TutorialTaskKind::Survive);
        assert_eq!(
            windowed_harness.controls.crosshair.position(),
            SubpixelPoint {
                x: WORLD_CENTER.x * 1024,
                y: WORLD_CENTER.y * 1024 - 3046,
            },
        );
    }

    #[test]
    fn advance_frame_ends_the_fullscreen_task_on_the_fullscreen_key_or_after_its_time() {
        let mut key_harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        let mut timed_harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        key_harness.start_task(TutorialTaskKind::Fullscreen);
        timed_harness.start_task(TutorialTaskKind::Fullscreen);

        key_harness.change_key(GameKeyKind::Fullscreen, KeyTransitionKind::Pressed);
        key_harness.run_ticks(1);
        timed_harness.run_ticks(FULLSCREEN_TASK_TICKS - 1);

        assert_eq!(key_harness.tutorial.task(), TutorialTaskKind::Survive);
        assert_eq!(timed_harness.tutorial.task(), TutorialTaskKind::Fullscreen);

        timed_harness.run_ticks(1);

        assert_eq!(timed_harness.tutorial.task(), TutorialTaskKind::Survive);
    }

    #[test]
    fn advance_frame_masks_the_presses_of_abilities_the_task_has_not_enabled() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.start_task(TutorialTaskKind::Survive);

        harness.press(GameKeyKind::FirstAbility);
        harness.run_ticks(1);

        assert!(harness.player_organism().abilities.first.is_ready());

        harness.run_ticks(SURVIVE_TASK_TICKS);
        harness.press(GameKeyKind::FirstAbility);
        harness.run_ticks(1);

        assert_eq!(harness.tutorial.task(), TutorialTaskKind::Extend);
        assert!(harness.player_organism().abilities.first.is_active());
    }

    #[test]
    fn advance_frame_ends_an_ability_task_when_the_ability_it_pressed_ends() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.start_task(TutorialTaskKind::Extend);

        harness.press(GameKeyKind::FirstAbility);
        harness.run_ticks(ability_constants::EXTEND_ACTIVE_TICKS);

        assert_eq!(harness.tutorial.task(), TutorialTaskKind::Extend);

        harness.run_ticks(1);

        assert_eq!(harness.tutorial.task(), TutorialTaskKind::Immortality);
    }

    #[test]
    fn start_task_writes_the_loadout_of_the_task() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);

        harness.start_task(TutorialTaskKind::Shoot);
        let shoot_loadout: Loadout = harness.tutorial.state().members[&TUTORIAL_PLAYER_ID].loadout.unwrap();
        harness.start_task(TutorialTaskKind::Toxin);
        let toxin_loadout: Loadout = harness.tutorial.state().members[&TUTORIAL_PLAYER_ID].loadout.unwrap();

        assert_eq!(
            (shoot_loadout.first, shoot_loadout.second, shoot_loadout.third),
            (
                FirstAbilityKind::Compress,
                SecondAbilityKind::Freeze,
                ThirdAbilityKind::Neutralize
            ),
        );
        assert_eq!(toxin_loadout.third, ThirdAbilityKind::Toxin);
    }

    #[test]
    fn start_task_places_the_bot_for_the_compress_task_away_from_the_player() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);

        harness.start_task(TutorialTaskKind::Compress);
        harness.run_ticks(1);

        let bot: TutorialBot = harness.bot();
        let bot_cursor: WorldPoint = harness.bot_organism_mut().cursor;

        assert_eq!(bot.member_id, MemberId(1));
        assert_eq!(bot_cursor, bot.cursor);
        assert!(
            bot_cursor.distance_squared(WORLD_CENTER)
                >= tutorial_bot::TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS
                    * tutorial_bot::TUTORIAL_BOT_MINIMUM_DISTANCE_PIXELS
        );
    }

    #[test]
    fn advance_task_ends_the_compress_task_once_the_compress_effect_on_the_bot_ends() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.start_task(TutorialTaskKind::Compress);
        harness.run_ticks(1);
        let bot_member_id: MemberId = harness.bot().member_id;
        harness.bot_organism_mut().last_hitter = Some(TUTORIAL_PLAYER_ID);

        harness.meet_condition(&[SimulationEvent::EffectApplied {
            target: bot_member_id,
            caster: TUTORIAL_PLAYER_ID,
            kind: ShotEffectKind::Freeze,
        }]);

        assert_eq!(harness.tutorial.task_deadline, None);

        harness.meet_condition(&[SimulationEvent::EffectApplied {
            target: bot_member_id,
            caster: TUTORIAL_PLAYER_ID,
            kind: ShotEffectKind::Compress,
        }]);
        harness.run_ticks(ability_constants::COMPRESS_EFFECT_TICKS);

        assert_eq!(harness.tutorial.task(), TutorialTaskKind::Freeze);
        assert_eq!(harness.bot_organism_mut().last_hitter, None);
    }

    #[test]
    fn advance_task_ends_the_toxin_task_once_the_player_hits_or_kills_the_bot() {
        let mut hit_harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        let mut kill_harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);

        for harness in [&mut hit_harness, &mut kill_harness] {
            harness.start_task(TutorialTaskKind::Compress);
            harness.run_ticks(1);
            harness.start_task(TutorialTaskKind::Toxin);
        }

        let bot_member_id: MemberId = kill_harness.bot().member_id;
        hit_harness.bot_organism_mut().last_hitter = Some(TUTORIAL_PLAYER_ID);
        hit_harness.meet_condition(&[]);
        kill_harness.meet_condition(&[SimulationEvent::OrganismDied {
            member_id: bot_member_id,
            credited_to: Some(TUTORIAL_PLAYER_ID),
        }]);

        for harness in [&mut hit_harness, &mut kill_harness] {
            harness.run_ticks(ability_constants::TOXIN_ACTIVE_TICKS);

            assert_eq!(harness.tutorial.task(), TutorialTaskKind::Spore);
        }
    }

    #[test]
    fn advance_frame_pauses_halfway_through_a_spore_flight_until_the_spores_secrete() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.run_ticks(40);
        harness.start_task(TutorialTaskKind::Spore);

        harness.press(GameKeyKind::FourthAbility);
        harness.run_ticks(1);

        assert!(matches!(
            harness.player_organism().abilities.spore,
            SporePhase::Flying { .. }
        ));

        harness.run_ticks(ability_constants::SPORE_FLIGHT_TICKS / 2);
        let paused_tick: Tick = harness.tutorial.state().tick;
        harness.run_ticks(20);

        assert_eq!(harness.tutorial.state().tick, paused_tick);
        assert_eq!(
            harness.tutorial.message_text(),
            "Reactivate the ability to cause all spores to secrete an acid, killing enemy cells",
        );

        harness.press(GameKeyKind::FourthAbility);
        harness.run_ticks(1);

        assert_eq!(harness.tutorial.state().tick, paused_tick.next());
        assert!(matches!(
            harness.player_organism().abilities.spore,
            SporePhase::Secreting { .. }
        ));
    }

    #[test]
    fn advance_task_ends_the_spore_task_after_a_secretion_hits_the_bot() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.start_task(TutorialTaskKind::Compress);
        harness.run_ticks(1);
        harness.start_task(TutorialTaskKind::Spore);
        harness.bot_organism_mut().last_hitter = Some(TUTORIAL_PLAYER_ID);

        harness.meet_condition(&[]);

        assert_eq!(harness.tutorial.task_deadline, None);

        harness
            .tutorial
            .local_game
            .state_mut()
            .members
            .get_mut(&TUTORIAL_PLAYER_ID)
            .unwrap()
            .organism
            .as_mut()
            .unwrap()
            .abilities
            .spore = SporePhase::Secreting {
            ends_at: Tick(500),
            spores: Vec::new(),
        };
        harness.meet_condition(&[]);
        harness.run_ticks(ability_constants::SPORE_SECRETION_TICKS);

        assert_eq!(harness.tutorial.task(), TutorialTaskKind::Done);
    }

    #[test]
    fn advance_frame_places_every_organism_again_at_its_own_cursor_after_it_dies() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.start_task(TutorialTaskKind::Compress);
        harness.run_ticks(1);
        let bot: TutorialBot = harness.bot();
        let crosshair_point: WorldPoint = WorldPoint { x: 120, y: 90 };
        harness.controls.crosshair.snap_to(crosshair_point.to_subpixel_point());

        for member_id in [TUTORIAL_PLAYER_ID, bot.member_id] {
            harness.tutorial.local_game.state_mut().members.get_mut(&member_id).unwrap().organism = None;
        }

        harness.run_ticks(1);

        assert_eq!(harness.player_organism().cursor, crosshair_point);
        assert_eq!(harness.bot_organism_mut().cursor, bot.cursor);
    }

    #[test]
    fn resize_keeps_the_crosshair_and_the_bot_at_their_place_in_the_world() {
        let mut harness: TutorialHarness = TutorialHarness::new(DisplayModeKind::Windowed);
        harness.start_task(TutorialTaskKind::Compress);
        harness.run_ticks(1);
        let bot_cursor: WorldPoint = harness.bot().cursor;

        harness.tutorial.resize(
            PixelSize {
                width_pixels: 1650,
                height_pixels: 1250,
            },
            &mut harness.controls,
        );

        assert_eq!(
            harness.controls.crosshair.position(),
            WorldPoint {
                x: WORLD_CENTER.x * 2,
                y: WORLD_CENTER.y * 2,
            }
            .to_subpixel_point(),
        );
        assert_eq!(
            harness.bot().cursor,
            WorldPoint {
                x: bot_cursor.x * 2,
                y: bot_cursor.y * 2,
            },
        );
        assert_eq!(harness.player_organism().cells.count(), 1);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p shared --lib tutorial::tutorial::`

Expected: compile failure, first line ``error[E0425]: cannot find type `PixelSize` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/tutorial/tutorial.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::ability::ability_constants;
use crate::ability::{
    AbilityPressSet, FirstAbilityKind, Loadout, SecondAbilityKind, ShotEffectKind, SporePhase, ThirdAbilityKind,
};
use crate::game::tick;
use crate::game::{GameState, MemberEvent, PlayerTickInput, SimulationEvent, Tick};
use crate::geometry::WorldPoint;
use crate::member::{Appearance, MemberId, OrganismColorKind, SkinKind};
use crate::organism::Organism;
use crate::play::{
    AbilityPressSample, FrameOutput, FramePacer, GameKeyKind, InputFocusKind, KeyboardState, LocalGame, LocalJoiner,
    PixelSize, PlayerControls,
};
use crate::play::{crosshair, local_game};
use crate::random::Pcg32;
use crate::tutorial::TutorialBot;
use crate::tutorial::tutorial_bot;

pub const FULLSCREEN_TASK_TICKS: u32 = tick::get_ticks_from_milliseconds(3500);
pub const SURVIVE_TASK_TICKS: u32 = tick::get_ticks_from_milliseconds(4500);
pub const SHOOT_TASK_TICKS: u32 = tick::get_ticks_from_milliseconds(10_000);
/// Bright colours which would hide the crosshair.
pub const TUTORIAL_PLAYER_EXCLUDED_COLORS: [OrganismColorKind; 2] = [OrganismColorKind::Sun, OrganismColorKind::Sky];
const TUTORIAL_PLAYER_ID: MemberId = MemberId(0);
const TUTORIAL_PLAYER_CAP: u8 = 2;
const TUTORIAL_PLAYER_SCREEN_NAME: &str = "Player";

/// In task order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TutorialTaskKind {
    Move,
    Fullscreen,
    Survive,
    Extend,
    Immortality,
    Neutralize,
    Shoot,
    Compress,
    Freeze,
    Toxin,
    Spore,
    Done,
}

impl TutorialTaskKind {
    pub fn next(self, display_mode: DisplayModeKind) -> TutorialTaskKind {
        match self {
            TutorialTaskKind::Move => match display_mode {
                DisplayModeKind::Windowed => TutorialTaskKind::Fullscreen,
                DisplayModeKind::Fullscreen => TutorialTaskKind::Survive,
            },
            TutorialTaskKind::Fullscreen => TutorialTaskKind::Survive,
            TutorialTaskKind::Survive => TutorialTaskKind::Extend,
            TutorialTaskKind::Extend => TutorialTaskKind::Immortality,
            TutorialTaskKind::Immortality => TutorialTaskKind::Neutralize,
            TutorialTaskKind::Neutralize => TutorialTaskKind::Shoot,
            TutorialTaskKind::Shoot => TutorialTaskKind::Compress,
            TutorialTaskKind::Compress => TutorialTaskKind::Freeze,
            TutorialTaskKind::Freeze => TutorialTaskKind::Toxin,
            TutorialTaskKind::Toxin => TutorialTaskKind::Spore,
            TutorialTaskKind::Spore | TutorialTaskKind::Done => TutorialTaskKind::Done,
        }
    }

    /// `None` for a task which ends on a condition.
    pub fn timed_task_ticks(self) -> Option<u32> {
        match self {
            TutorialTaskKind::Fullscreen => Some(FULLSCREEN_TASK_TICKS),
            TutorialTaskKind::Survive => Some(SURVIVE_TASK_TICKS),
            TutorialTaskKind::Shoot => Some(SHOOT_TASK_TICKS),
            TutorialTaskKind::Move
            | TutorialTaskKind::Extend
            | TutorialTaskKind::Immortality
            | TutorialTaskKind::Neutralize
            | TutorialTaskKind::Compress
            | TutorialTaskKind::Freeze
            | TutorialTaskKind::Toxin
            | TutorialTaskKind::Spore
            | TutorialTaskKind::Done => None,
        }
    }

    /// The original's per-task `activated` toggles.
    pub fn enabled_abilities(self) -> AbilityPressSet {
        match self {
            TutorialTaskKind::Move
            | TutorialTaskKind::Fullscreen
            | TutorialTaskKind::Survive
            | TutorialTaskKind::Done => AbilityPressSet::NONE,
            TutorialTaskKind::Extend | TutorialTaskKind::Compress => AbilityPressSet::FIRST,
            TutorialTaskKind::Immortality | TutorialTaskKind::Freeze => AbilityPressSet::SECOND,
            TutorialTaskKind::Neutralize | TutorialTaskKind::Toxin => AbilityPressSet::THIRD,
            TutorialTaskKind::Shoot => AbilityPressSet::FIRST.with(AbilityPressSet::SECOND),
            TutorialTaskKind::Spore => AbilityPressSet::FOURTH,
        }
    }

    pub fn first_ability(self) -> FirstAbilityKind {
        if self >= TutorialTaskKind::Shoot {
            return FirstAbilityKind::Compress;
        }

        FirstAbilityKind::Extend
    }

    pub fn second_ability(self) -> SecondAbilityKind {
        if self >= TutorialTaskKind::Shoot {
            return SecondAbilityKind::Freeze;
        }

        SecondAbilityKind::Immortality
    }

    pub fn third_ability(self) -> ThirdAbilityKind {
        if self >= TutorialTaskKind::Toxin {
            return ThirdAbilityKind::Toxin;
        }

        ThirdAbilityKind::Neutralize
    }

    pub fn is_hud_visible(self) -> bool {
        !matches!(self, TutorialTaskKind::Move | TutorialTaskKind::Survive)
    }

    /// Verbatim from the original's `Message.js`.
    pub fn message_text(self, stepping: TutorialSteppingKind) -> &'static str {
        match (self, stepping) {
            (TutorialTaskKind::Move, _) => "Use W-A-S-D (Recommended) or the arrow keys to move",
            (TutorialTaskKind::Fullscreen, _) => "Press F11 to enter fullscreen mode (Recommended)",
            (TutorialTaskKind::Survive, _) => "If the crosshair is too far from the organism, it will die",
            (TutorialTaskKind::Extend, _) => "Use the EXTEND ability to increase the organism's size",
            (TutorialTaskKind::Immortality, _) => "The IMMORTALITY ability will stop the natural atrophe of cells",
            (TutorialTaskKind::Neutralize, _) => "NEUTRALIZE will create a bubble of safety from enemy attacks",
            (TutorialTaskKind::Shoot, _) => {
                "To COMPRESS or FREEZE an enemy, press the ability key to launch a spore in the direction of the cursor\nThen press it again to activate the ability"
            }
            (TutorialTaskKind::Compress, _) => {
                "On hit, COMPRESS shrinks the size of the targeted enemy\nCOMPRESS the bot to progress"
            }
            (TutorialTaskKind::Freeze, _) => {
                "On hit, FREEZE halts all natural processes within the enemy organism\nFREEZE the bot to progress"
            }
            (TutorialTaskKind::Toxin, _) => {
                "TOXIN creates a localized bubble in which only you can survive\nDamage the bot to progress"
            }
            (TutorialTaskKind::Spore, TutorialSteppingKind::Running) => {
                "Use SPORE to jettison outer cells in all directions (Space Bar)\nDamage the bot to progress"
            }
            (TutorialTaskKind::Spore, TutorialSteppingKind::PausedForSecretion) => {
                "Reactivate the ability to cause all spores to secrete an acid, killing enemy cells"
            }
            (TutorialTaskKind::Done, _) => "Now that you're ready, press ESC to return to the menu",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayModeKind {
    Windowed,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TutorialSteppingKind {
    Running,
    /// Halfway through a spore flight in the spore task, until the spores are made to secrete.
    PausedForSecretion,
}

/// A local game teaching the controls one task at a time.
#[derive(Clone, Debug, PartialEq)]
pub struct Tutorial {
    local_game: LocalGame,
    pacer: FramePacer,
    scene_rng: Pcg32,
    task: TutorialTaskKind,
    /// The tick at which the task ends: set on entry to a timed task, or when the task's condition is met.
    task_deadline: Option<Tick>,
    stepping: TutorialSteppingKind,
    display_mode: DisplayModeKind,
    bot: Option<TutorialBot>,
    /// Applied in the next local tick.
    pending_member_events: Vec<MemberEvent>,
}

impl Tutorial {
    /// The player starts at the world centre, with the crosshair on it.
    pub fn new(
        window_size: PixelSize,
        seed: u64,
        display_mode: DisplayModeKind,
        controls: &mut PlayerControls,
    ) -> Tutorial {
        let mut scene_rng: Pcg32 = local_game::create_scene_rng(seed);
        let world_size: PixelSize = local_game::get_world_size(window_size);
        let player_cursor: WorldPoint = WorldPoint {
            x: i32::try_from(world_size.width_pixels / 2).unwrap_or(0),
            y: i32::try_from(world_size.height_pixels / 2).unwrap_or(0),
        };
        let player_joiner: LocalJoiner = LocalJoiner {
            screen_name: String::from(TUTORIAL_PLAYER_SCREEN_NAME),
            appearance: Appearance {
                color: local_game::draw_color_excluding(&mut scene_rng, &TUTORIAL_PLAYER_EXCLUDED_COLORS),
                skin: SkinKind::None,
            },
            cursor: player_cursor,
        };

        controls.crosshair.snap_to(player_cursor.to_subpixel_point());

        Tutorial {
            local_game: LocalGame::new(window_size, TUTORIAL_PLAYER_CAP, seed, &[player_joiner]),
            pacer: FramePacer::new(),
            scene_rng,
            task: TutorialTaskKind::Move,
            task_deadline: None,
            stepping: TutorialSteppingKind::Running,
            display_mode,
            bot: None,
            pending_member_events: Vec::new(),
        }
    }

    pub fn state(&self) -> &GameState {
        self.local_game.state()
    }

    pub fn task(&self) -> TutorialTaskKind {
        self.task
    }

    pub fn message_text(&self) -> &'static str {
        self.task.message_text(self.stepping)
    }

    pub fn is_hud_visible(&self) -> bool {
        self.task.is_hud_visible()
    }

    pub fn display_mode_changed(&mut self, display_mode: DisplayModeKind) {
        self.display_mode = display_mode;
    }

    pub fn advance_frame(
        &mut self,
        now_milliseconds: f64,
        controls: &mut PlayerControls,
        input_focus: InputFocusKind,
    ) -> FrameOutput {
        let frame_duration_milliseconds: f64 = self.pacer.begin_frame(now_milliseconds);
        let is_steering: bool = input_focus == InputFocusKind::Game;

        if is_steering && self.stepping == TutorialSteppingKind::Running {
            controls.crosshair.advance(
                &controls.keyboard,
                frame_duration_milliseconds,
                crosshair::CROSSHAIR_SPEED_PIXELS_PER_SECOND,
            );
        }

        if is_steering {
            self.check_held_key_tasks(&controls.keyboard);
        }

        let local_tick_count: u32 = self.pacer.take_local_tick_count(frame_duration_milliseconds);
        let steppable_tick_count: u32 = self.take_steppable_tick_count(local_tick_count, &controls.keyboard);
        let mut tick_applied: Option<Tick> = None;

        for _ in 0..steppable_tick_count {
            self.step_tick(controls);
            tick_applied = Some(self.state().tick);
        }

        if tick_applied.is_some() {
            self.pacer.record_tick_applied(now_milliseconds);
        }

        FrameOutput {
            interpolation_alpha: self.pacer.get_interpolation_alpha(now_milliseconds),
            tick_applied,
            effects: Vec::new(),
        }
    }

    /// The crosshair and the bot's cursor keep their place in the world; every organism restarts as one cell.
    pub fn resize(&mut self, window_size: PixelSize, controls: &mut PlayerControls) {
        let old_size: PixelSize = self.local_game.world_size();
        self.local_game.resize(window_size);
        let new_size: PixelSize = self.local_game.world_size();

        let crosshair_point: WorldPoint = controls.crosshair.position().to_rounded_world_point();
        let rescaled_crosshair_point: WorldPoint = local_game::rescale_point(crosshair_point, old_size, new_size);
        controls.crosshair.snap_to(rescaled_crosshair_point.to_subpixel_point());

        if let Some(bot) = self.bot.as_mut() {
            bot.cursor = local_game::rescale_point(bot.cursor, old_size, new_size);
        }
    }

    fn check_held_key_tasks(&mut self, keyboard: &KeyboardState) {
        let is_task_done: bool = match self.task {
            TutorialTaskKind::Move => keyboard.is_any_movement_key_held(),
            TutorialTaskKind::Fullscreen => keyboard.is_held(GameKeyKind::Fullscreen),
            TutorialTaskKind::Survive
            | TutorialTaskKind::Extend
            | TutorialTaskKind::Immortality
            | TutorialTaskKind::Neutralize
            | TutorialTaskKind::Shoot
            | TutorialTaskKind::Compress
            | TutorialTaskKind::Freeze
            | TutorialTaskKind::Toxin
            | TutorialTaskKind::Spore
            | TutorialTaskKind::Done => false,
        };

        if is_task_done {
            self.start_task(self.task.next(self.display_mode));
        }
    }

    /// While paused for secretion, no tick is stepped until the secreting press is pending.
    fn take_steppable_tick_count(&mut self, local_tick_count: u32, keyboard: &KeyboardState) -> u32 {
        if self.stepping == TutorialSteppingKind::Running {
            return local_tick_count;
        }

        let is_secretion_pressed: bool = keyboard.pending_ability_presses().contains(AbilityPressSet::FOURTH);

        if !is_secretion_pressed {
            return 0;
        }

        self.stepping = TutorialSteppingKind::Running;

        local_tick_count
    }

    fn step_tick(&mut self, controls: &mut PlayerControls) {
        let sample: AbilityPressSample = controls.keyboard.take_ability_presses().masked(self.task.enabled_abilities());
        let player_cursor: WorldPoint = controls.crosshair.position().to_rounded_world_point();
        let player_input: PlayerTickInput = PlayerTickInput {
            member_id: TUTORIAL_PLAYER_ID,
            cursor: player_cursor,
            ability_presses: sample.ability_presses,
            aim: sample.aim,
        };
        let member_events: Vec<MemberEvent> = std::mem::take(&mut self.pending_member_events);

        let simulation_events: Vec<SimulationEvent> =
            self.local_game.step_local_tick(member_events, vec![player_input]);

        self.advance_task(&sample, &simulation_events);
        self.place_missing_organisms(player_cursor);
        self.pause_for_secretion_when_due();
    }

    fn advance_task(&mut self, sample: &AbilityPressSample, simulation_events: &[SimulationEvent]) {
        let tick: Tick = self.state().tick;

        if self.task_deadline.is_none() {
            let condition_ticks: Option<u32> = self.get_task_condition_ticks(sample, simulation_events);
            self.task_deadline = condition_ticks.map(|task_ticks| tick.plus(task_ticks));
        }

        let is_task_due: bool = self.task_deadline.is_some_and(|task_deadline| task_deadline <= tick);

        if is_task_due {
            self.start_task(self.task.next(self.display_mode));
        }
    }

    /// The ticks until the task ends, once its condition is met; each is the duration of what the condition started.
    fn get_task_condition_ticks(
        &self,
        sample: &AbilityPressSample,
        simulation_events: &[SimulationEvent],
    ) -> Option<u32> {
        let ability_presses: AbilityPressSet = sample.ability_presses;

        match self.task {
            TutorialTaskKind::Extend => {
                ability_presses.contains(AbilityPressSet::FIRST).then_some(ability_constants::EXTEND_ACTIVE_TICKS)
            }
            TutorialTaskKind::Immortality => ability_presses
                .contains(AbilityPressSet::SECOND)
                .then_some(ability_constants::IMMORTALITY_ACTIVE_TICKS),
            TutorialTaskKind::Neutralize => ability_presses
                .contains(AbilityPressSet::THIRD)
                .then_some(ability_constants::NEUTRALIZE_ACTIVE_TICKS),
            TutorialTaskKind::Compress => self
                .was_effect_applied_to_bot(simulation_events, ShotEffectKind::Compress)
                .then_some(ability_constants::COMPRESS_EFFECT_TICKS),
            TutorialTaskKind::Freeze => self
                .was_effect_applied_to_bot(simulation_events, ShotEffectKind::Freeze)
                .then_some(ability_constants::FREEZE_EFFECT_TICKS),
            TutorialTaskKind::Toxin => {
                self.was_bot_hit_by_player(simulation_events).then_some(ability_constants::TOXIN_ACTIVE_TICKS)
            }
            TutorialTaskKind::Spore => (self.is_player_secreting() && self.was_bot_hit_by_player(simulation_events))
                .then_some(ability_constants::SPORE_SECRETION_TICKS),
            TutorialTaskKind::Move
            | TutorialTaskKind::Fullscreen
            | TutorialTaskKind::Survive
            | TutorialTaskKind::Shoot
            | TutorialTaskKind::Done => None,
        }
    }

    fn start_task(&mut self, task: TutorialTaskKind) {
        self.task = task;
        self.task_deadline = task.timed_task_ticks().map(|task_ticks| self.state().tick.plus(task_ticks));
        self.write_player_loadout();

        match task {
            TutorialTaskKind::Compress => self.add_bot(),
            TutorialTaskKind::Freeze | TutorialTaskKind::Toxin | TutorialTaskKind::Spore => {
                self.clear_bot_last_hitter()
            }
            TutorialTaskKind::Move
            | TutorialTaskKind::Fullscreen
            | TutorialTaskKind::Survive
            | TutorialTaskKind::Extend
            | TutorialTaskKind::Immortality
            | TutorialTaskKind::Neutralize
            | TutorialTaskKind::Shoot
            | TutorialTaskKind::Done => {}
        }
    }

    fn write_player_loadout(&mut self) {
        let task: TutorialTaskKind = self.task;
        let Some(player) = self.local_game.state_mut().members.get_mut(&TUTORIAL_PLAYER_ID) else {
            return;
        };

        let Some(loadout) = player.loadout.as_mut() else {
            return;
        };

        *loadout = Loadout {
            appearance: loadout.appearance,
            first: task.first_ability(),
            second: task.second_ability(),
            third: task.third_ability(),
        };
    }

    /// Joins in the next tick and is placed after it.
    fn add_bot(&mut self) {
        if self.bot.is_some() {
            return;
        }

        let player_cursor: WorldPoint =
            self.get_player_organism().map_or(WorldPoint { x: 0, y: 0 }, |organism| organism.cursor);
        let bot_joiner: LocalJoiner =
            tutorial_bot::create_tutorial_bot_joiner(&mut self.scene_rng, self.local_game.world_size(), player_cursor);
        let member_id: MemberId = self.state().next_member_id;

        self.pending_member_events.push(local_game::create_joined_event(member_id, &bot_joiner));
        self.bot = Some(TutorialBot {
            member_id,
            cursor: bot_joiner.cursor,
        });
    }

    fn clear_bot_last_hitter(&mut self) {
        let Some(bot) = self.bot else {
            return;
        };

        let bot_organism: Option<&mut Organism> = self
            .local_game
            .state_mut()
            .members
            .get_mut(&bot.member_id)
            .and_then(|member| member.organism.as_mut());

        if let Some(bot_organism) = bot_organism {
            bot_organism.last_hitter = None;
        }
    }

    /// The player at the crosshair, the bot at its own cursor.
    fn place_missing_organisms(&mut self, player_cursor: WorldPoint) {
        let mut cursors: Vec<(MemberId, WorldPoint)> = vec![(TUTORIAL_PLAYER_ID, player_cursor)];
        cursors.extend(self.bot.map(|bot| (bot.member_id, bot.cursor)));

        self.local_game.place_missing_organisms(&cursors);
    }

    fn pause_for_secretion_when_due(&mut self) {
        if self.task != TutorialTaskKind::Spore || self.stepping != TutorialSteppingKind::Running {
            return;
        }

        let Some(SporePhase::Flying { ends_at, .. }) =
            self.get_player_organism().map(|organism| &organism.abilities.spore)
        else {
            return;
        };

        let remaining_flight_ticks: u32 = ends_at.ticks_since(self.state().tick);
        let elapsed_flight_ticks: u32 = ability_constants::SPORE_FLIGHT_TICKS.saturating_sub(remaining_flight_ticks);

        if elapsed_flight_ticks >= ability_constants::SPORE_FLIGHT_TICKS / 2 {
            self.stepping = TutorialSteppingKind::PausedForSecretion;
        }
    }

    fn was_effect_applied_to_bot(&self, simulation_events: &[SimulationEvent], effect_kind: ShotEffectKind) -> bool {
        let Some(bot) = self.bot else {
            return false;
        };

        simulation_events.iter().any(|simulation_event| {
            matches!(
                simulation_event,
                SimulationEvent::EffectApplied { target, caster, kind }
                    if *target == bot.member_id && *caster == TUTORIAL_PLAYER_ID && *kind == effect_kind
            )
        })
    }

    /// A hit which killed the bot shows only in the death's kill credit.
    fn was_bot_hit_by_player(&self, simulation_events: &[SimulationEvent]) -> bool {
        let Some(bot) = self.bot else {
            return false;
        };

        let is_last_hitter: bool = self
            .state()
            .members
            .get(&bot.member_id)
            .and_then(|member| member.organism.as_ref())
            .is_some_and(|organism| organism.last_hitter == Some(TUTORIAL_PLAYER_ID));
        let was_killed_by_player: bool = simulation_events.iter().any(|simulation_event| {
            matches!(
                simulation_event,
                SimulationEvent::OrganismDied { member_id, credited_to: Some(TUTORIAL_PLAYER_ID) }
                    if *member_id == bot.member_id
            )
        });

        is_last_hitter || was_killed_by_player
    }

    fn is_player_secreting(&self) -> bool {
        self.get_player_organism()
            .is_some_and(|organism| matches!(organism.abilities.spore, SporePhase::Secreting { .. }))
    }

    fn get_player_organism(&self) -> Option<&Organism> {
        self.state().members.get(&TUTORIAL_PLAYER_ID)?.organism.as_ref()
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared --lib tutorial::tutorial::`

Expected: `test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 431 filtered out`.

- [ ] **Step 5: Run the forbidden-operations scan, which covers `tutorial/` and `title/`**

Run: `cargo test -p shared --test forbidden_operations`

Expected: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 6: Commit**

```sh
git status
git add shared/src/tutorial/tutorial.rs
git commit -m "tutorial"
```

### Task 18: Active scene

**Files:**
- Modify: `shared/src/play/active_scene.rs`
- Test: inline `mod tests` in `shared/src/play/active_scene.rs`

`ActiveScene { Online(ClientSession), Tutorial(Tutorial), Title(TitleScene) }` with the `advance_frame` dispatch the platform calls every frame, and the state each scene renders (client-web.md, client session, frame pacing and driving).

- [ ] **Step 1: Write the failing tests**

Append to the end of `shared/src/play/active_scene.rs`, after one blank line:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Tick;
    use crate::geometry::SubpixelPoint;
    use crate::play::{GameKeyKind, KeyRepeatKind, KeyTransitionKind, PixelSize};
    use crate::tutorial::DisplayModeKind;

    const WINDOW_SIZE: PixelSize = PixelSize {
        width_pixels: 850,
        height_pixels: 650,
    };

    #[test]
    fn advance_frame_steps_the_title_scene() {
        let mut controls: PlayerControls = PlayerControls::new();
        let mut active_scene: ActiveScene = ActiveScene::Title(TitleScene::new(WINDOW_SIZE, 3));

        active_scene.advance_frame(0.0, &mut controls, InputFocusKind::Menu);
        let frame_output: FrameOutput = active_scene.advance_frame(70.0, &mut controls, InputFocusKind::Menu);

        assert_eq!(frame_output.tick_applied, Some(Tick(2)));
        assert_eq!(active_scene.render_state().map(|state| state.tick), Some(Tick(2)));
    }

    #[test]
    fn advance_frame_steers_the_tutorial_crosshair_with_the_game_focus() {
        let mut controls: PlayerControls = PlayerControls::new();
        let tutorial: Tutorial = Tutorial::new(WINDOW_SIZE, 3, DisplayModeKind::Windowed, &mut controls);
        let start: SubpixelPoint = controls.crosshair.position();
        let mut active_scene: ActiveScene = ActiveScene::Tutorial(tutorial);
        controls.keyboard.key_changed(
            GameKeyKind::MoveRight,
            KeyTransitionKind::Pressed,
            KeyRepeatKind::Initial,
        );

        active_scene.advance_frame(0.0, &mut controls, InputFocusKind::Game);
        active_scene.advance_frame(70.0, &mut controls, InputFocusKind::Game);

        assert_eq!(
            controls.crosshair.position(),
            SubpixelPoint {
                x: start.x + 3046,
                y: start.y,
            },
        );
    }

    #[test]
    fn render_state_is_none_for_an_online_scene_outside_a_game() {
        let active_scene: ActiveScene = ActiveScene::Online(ClientSession::new());

        assert_eq!(active_scene.render_state(), None);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p shared --lib play::active_scene`

Expected: compile failure, first line ``error[E0425]: cannot find type `PlayerControls` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/play/active_scene.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::game::GameState;
use crate::play::{ClientSession, FrameOutput, PlayerControls};
use crate::title::TitleScene;
use crate::tutorial::Tutorial;

/// Whether keyboard input steers the game or a menu over it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputFocusKind {
    Game,
    Menu,
}

/// The scene the platform advances every frame and renders.
#[derive(Clone, Debug, PartialEq)]
pub enum ActiveScene {
    Online(ClientSession),
    Tutorial(Tutorial),
    Title(TitleScene),
}

impl ActiveScene {
    pub fn advance_frame(
        &mut self,
        now_milliseconds: f64,
        controls: &mut PlayerControls,
        input_focus: InputFocusKind,
    ) -> FrameOutput {
        match self {
            ActiveScene::Online(client_session) => {
                client_session.advance_frame(now_milliseconds, controls, input_focus)
            }
            ActiveScene::Tutorial(tutorial) => tutorial.advance_frame(now_milliseconds, controls, input_focus),
            ActiveScene::Title(title_scene) => title_scene.advance_frame(now_milliseconds),
        }
    }

    /// `None` while an online scene has no game state.
    pub fn render_state(&self) -> Option<&GameState> {
        match self {
            ActiveScene::Online(client_session) => client_session.render_state(),
            ActiveScene::Tutorial(tutorial) => Some(tutorial.state()),
            ActiveScene::Title(title_scene) => Some(title_scene.state()),
        }
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared --lib play::active_scene`

Expected: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 451 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/play/active_scene.rs
git commit -m "active scene"
```

### Task 19: Architecture docs for the client session

**Files:**
- Modify: `docs/architecture/overview.md`
- Modify: `docs/architecture/client-web.md`
- Modify: `docs/architecture/testing.md`

The architecture keeps describing the code: the new `play/local_game.rs` and the filled tree entries, `ConnectionLost(Option<CloseReasonKind>)`, and the headless test. Documentation only; the verification is a search for the new text.

- [ ] **Step 1: Edit the module tree**

In `docs/architecture/overview.md`, replace:

```text
                  interpolation.rs       projectile interpolation
                  message_text.rs        get_message_text
tutorial/         tutorial.rs            Tutorial, TutorialTaskKind
                  tutorial_bot.rs
title/            title_scene.rs         TitleScene (10 dummy organisms)
settings/         settings.rs            SettingKey, SettingValue, SettingsStore, Setting (eafora pattern)
```

with:

```text
                  interpolation.rs       projectile interpolation
                  local_game.rs          LocalGame, LocalJoiner, PixelSize (local settings, stepping, placement, resize)
                  message_text.rs        get_message_text, TransientMessageKind
tutorial/         tutorial.rs            Tutorial, TutorialTaskKind
                  tutorial_bot.rs        TutorialBot, bot placement
title/            title_scene.rs         TitleScene (10 dummy organisms)
settings/         settings.rs            SettingKey, SettingValue, SettingsStore, Setting (eafora pattern), ClientSettingKind
```

- [ ] **Step 2: Edit the client session**

In `docs/architecture/client-web.md`, replace:

```text
  - `fn render_state(&self) -> Option<&GameState>`; `fn own_member_state(&self) -> Option<OwnMemberStateKind>`.
```

with:

```text
  - `fn render_state(&self) -> Option<&GameState>`; `fn own_member_state(&self) -> Option<OwnMemberStateKind>`.
  - `fn advance_frame(&mut self, now_milliseconds: f64, controls: &mut PlayerControls, input_focus: InputFocusKind) -> FrameOutput` (the `Online` arm of `ActiveScene::advance_frame`); `fn game_id(&self) -> Option<GameId>`; `fn own_member_id(&self) -> Option<MemberId>`; `fn message_text(&self) -> Option<String>` (the game message, or "Catching up" during a resync).
```

In `docs/architecture/client-web.md`, replace:

```text
  - `LeftGame`, `GameEnded`, `SynchronisationLost`, `ConnectionLost(CloseReasonKind)`.
```

with:

```text
  - `LeftGame`, `GameEnded`, `SynchronisationLost`, `ConnectionLost(Option<CloseReasonKind>)` (`None` for an abnormal close, a failed open or an unknown close code).
```

- [ ] **Step 3: Edit the server integration tests**

In `docs/architecture/testing.md`, replace:

```text
shared helper `tests/helpers/test_server.rs` (used by both files below).
```

with:

```text
shared helper `tests/helpers/test_server.rs` (used by every file below).
```

In `docs/architecture/testing.md`, replace:

```text
- Clamp: an input jump beyond the budget yields `CursorCorrected` and a clamped bundle cursor.
```

with:

```text
- Clamp: an input jump beyond the budget yields `CursorCorrected` and a clamped bundle cursor.
- `client_session.rs`: two `ClientSession`s, driven headless as a browser drives them (frames in, `SendFrame` out, `advance_frame` per display frame), play one game and compute identical checksums for every tick both applied, matching the server's `StateChecksum`s, without a resync.
```

- [ ] **Step 4: Verify the edits**

Run: `grep -c "local_game.rs\|ConnectionLost(Option<CloseReasonKind>)\|fn own_member_id(&self)\|used by every file below\|play one game and compute identical checksums" docs/architecture/overview.md docs/architecture/client-web.md docs/architecture/testing.md`

Expected, one line per file in any order:

```text
docs/architecture/overview.md:1
docs/architecture/client-web.md:2
docs/architecture/testing.md:2
```

- [ ] **Step 5: Commit**

```sh
git status
git add docs/architecture/overview.md docs/architecture/client-web.md docs/architecture/testing.md
git commit -m "architecture docs for the client session"
```

### Task 20: Phase verification

**Files:** none; verification only.

The roadmap's done condition for this phase: the client and tutorial unit tests pass, and the headless test drives two `ClientSession`s against the in-process server to identical checksums, with the rest of the workspace still building and passing for every target.

- [ ] **Step 1: Check formatting**

Run: `cargo fmt --all -- --check`

Expected: no output.

- [ ] **Step 2: Run every test of the workspace**

Run: `cargo test --workspace 2>&1 | grep -E '^test result|FAILED'`

Expected output, each line followed by its `finished in` timing:

```text
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 120 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 13 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 454 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Every line reads `ok`; in order, the binaries are `protocol_dump`, the `server` library, the `server` binary, `client_session`, `game_lifecycle`, `protocol_errors`, the `shared` library, `determinism`, `forbidden_operations`, `growth_statistics`, `protocol`, `replay`, the two `web` targets and the doc tests of `server`, `shared` and `web`.

- [ ] **Step 3: Check the wasm32 builds**

Run: `./scripts/test/check-wasm.sh`

Expected: exit status 0, with no `warning` or `error` line.

- [ ] **Step 4: Confirm the working tree is clean**

Run: `git status --short`

Expected: no output other than `?? .idea/`, the untracked IDE directory.

- [ ] **Step 5: List the phase's commits**

Run: `git log --oneline --reverse $(git log --grep '^>>> branch: client-session' --format=%h)..HEAD | cut -d' ' -f2-`

Expected, oldest first (review-fix commits, if any, sit after their task):

```text
phase 6 plan
phase 6 plan review
subpixel rounding in shared geometry
ability press set intersection
client settings
game keys and press edges
crosshair controller and player controls
frame pacer
projectile interpolation
own member state
message text
client session frames
client session synchronisation
two client sessions against the server
local game
title scene
tutorial bot
tutorial tasks
tutorial
active scene
architecture docs for the client session
```

## Roadmap coverage

- `shared/src/play/`:
  - client session: Tasks 10 and 11 (`ClientSession`, `ClientEffect`, snapshots, bundles, checksums, resync, backlog, re-request, resync limit, inputs, corrections);
  - frame pacing: Task 6, with `FrameOutput` in Task 11;
  - crosshair: Task 5 (`CrosshairController`, `SentCursorHistory`, `PlayerControls`);
  - keys: Task 4 (`GameKeyKind`, `KeyboardState`, press edges);
  - own member state: Task 8;
  - message text: Task 9;
  - interpolation: Task 7;
  - active scene: Task 11 (`InputFocusKind`) and Task 18 (`ActiveScene`);
  - local games: Task 13.
- `tutorial/`: Tasks 15, 16 and 17.
- `title/`: Task 14.
- `settings/`: Task 3.
- Done condition:
  - testing.md, client and server unit tests, client rows: `play/client_session.rs` (Tasks 10 and 11: snapshot then bundles; mismatch resync discarding bundles until the snapshot; undecodable and unconvertible bundle and snapshot; backlog; response timeout re-send; resync limit; no `Input` after a snapshot alone), `play/crosshair.rs` (Task 5), `play/game_key.rs` (Task 4), `play/frame_pacing.rs` (Task 6), `play/message_text.rs` (Task 9), `tutorial/tutorial.rs` (Tasks 16 and 17: task progression, ability mask per task, bot minimum distance with Task 15, per-organism replacement on death);
  - two `ClientSession`s against the in-process server to identical checksums: Task 12;
  - all checked together in Task 20.
- Left to later phases: the renderer and `RenderScene` over `InterpolationFrame` (phase 7); the web driver, the socket, `LocalStorageSettingsStore`, the screens and overlays which consume `ClientEffect`, `message_text`, `TransientMessageKind` and `get_respawn_key_outcome` (phase 8).

## PR description

**Problem:** bacter-rs had a simulation, a protocol and a server, but no client logic to follow a server game, steer a crosshair or run the title scene and the tutorial.

**Solution:** `shared/src/play/`: a sans-IO `ClientSession` which applies snapshots and bundles, checks checksums, resyncs, samples keyboard edges and the crosshair into inputs and rebases the crosshair on server corrections; the frame pacer, own member state, message texts and projectile interpolation; `LocalGame`, on which the title scene of `shared/src/title/` and the tutorial of `shared/src/tutorial/` run; the settings store contract in `shared/src/settings/`; `ActiveScene` to advance whichever scene is active. A headless test plays one game with two sessions against the in-process server to identical checksums. Repository: `bacter-rs` only.

## Deviations from the plan

- Files modified beyond the file structure (Task 8 review): `server/src/game/membership.rs`, `shared/src/game/game_state.rs`, `shared/src/game/game_summary.rs`, `shared/src/game/test_fixture.rs`, `shared/src/organism/spawn.rs`, `shared/src/protocol/game_state_serial.rs`, `shared/src/round/round.rs`.
- New shared functions not in the plan:
  - `round::is_round_in_progress`, replacing the private copies in `membership.rs`, `spawn.rs` and the plan's `is_spawn_allowed_by_round` in `own_member_state.rs`;
  - `GameState::is_at_player_cap`, used by `spawn.rs` and `get_respawn_key_outcome`;
  - `test_fixture::create_spectator`, replacing the ad hoc spectators of five test modules (the spectator in the `round.rs` test now has no loadout);
  - `Organism::reset_to_one_cell_at`, which `Organism::new` and `LocalGame::resize` use in place of the plan's private `reset_to_one_cell` in `local_game.rs`; the `GameState::settings` doc reads "Immutable during stepping", since `LocalGame::resize` edits the settings;
  - `local_game::draw_cell_coordinate_or_centre` and `local_game::get_world_centre`, shared by the title scene, the tutorial bot and the tutorial in place of three local centre fallbacks;
  - `COLOR_WHEN_EVERY_COLOR_IS_EXCLUDED` in `local_game.rs`.
- Changed signatures:
  - `local_game::draw_cell_coordinate` returns `Option<i32>` (`None` for a range with no clear pixel) instead of falling back inside;
  - `LocalGame::resize` keeps the world shape from the settings instead of always `Rectangle`;
  - the tutorial's `check_held_key_tasks`, `advance_task`, `start_task` and `add_bot` take the player's cursor (the rounded crosshair), so the bot is placed away from the crosshair rather than from the player's organism;
  - `ClientSession`'s snapshot re-send and unreported-tick logic moved into `GameSession::take_overdue_snapshot_request` and `GameSession::take_unreported_tick` (the plan text was updated in Task 11).
- Behaviour: `Tutorial::advance_frame` stops a multi-tick frame at the tick which pauses for secretion, so a long frame cannot step past the halfway point of a spore flight.
- Tests added beyond the plan: `key_changed_releases_a_held_key`, `receive_does_not_restart_a_resync_already_in_progress`, `receive_ignores_a_checksum_for_a_tick_other_than_the_applied_tick`, `draw_color_excluding_falls_back_when_every_color_is_excluded`, `draw_cell_coordinate_or_centre_falls_back_to_the_centre_of_a_range_too_short_for_a_cell`, `advance_frame_stops_a_multi_tick_frame_at_the_halfway_tick_of_a_spore_flight`; the tutorial's ability kind test is split into one test per slot. The `shared` library runs 462 tests, against the 454 Task 20 expects.
- Test helpers: the tutorial harness has `bot_organism`, `player_organism_mut` and `run_frame_spanning_ticks`; the headless test's effect match lists every `ClientEffect` and panics on `RequestRejected`, `ConnectionLost`, `LeftGame` and `GameEnded`.
- Named constants replace literals in tests and in `title_scene.rs` (`QUADRANT_COUNT`, a compile-time check that the quadrant limit fits the dummy count, `TITLE_PLAYER_CAP` from `TITLE_DUMMY_COUNT`).
- The code as transcribed was not entirely `rustfmt`-clean; a "phase 6 formatting" commit fixed it.
- Final review: the member cursor collections of `local_game.rs` are named `member_cursors`, and `Tutorial::write_player_loadout` binds the player's loadout before matching on it.
- Unnecessary steps: none.
