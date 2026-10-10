# Phase 4: protocol implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** the protocol layer of `shared` and the `protocol_dump` tool: every wire type with its conversions, the client and server messages and their codec, settings and text validation, the protocol limits, the rejection and close-reason kinds, the replay format and the snapshot, proven by round-trip, model-to-wire-to-model, malformed-input, size-budget and replay fixture tests.

**Architecture:** every wire type lives in `shared/src/protocol/` and is followed by its conversions (`docs/architecture/protocol.md#wire-types`, `docs/conventions/types.md`): `From<&Model>` to send, `TryFrom` to receive, with `RejectionKind` errors for request payloads and `AppError` for everything else; `protocol/protocol.rs` holds the only bitcode calls. The new `replay` module frames records around that codec and re-runs logs through `step`. `tools/protocol_dump` decodes captured frames and replay files on the command line.

**Tech Stack:** Rust 1.95 (edition 2024), bitcode 0.6 (derives, `encode`, `decode`; `shared/src/protocol/` only), minimer 2.2 (`AppError`), clap 4.6 (builder API, `protocol_dump`); no new dependencies.

---

## Ground rules for the executor

- Repository `/Users/singularity/bacter-rs`, branch `protocol`. Run `git -C /Users/singularity/bacter-rs branch --show-current` first; stop if it does not print `protocol`.
- Every command runs from the repository root `/Users/singularity/bacter-rs`.
- Never switch branches, push, rebase or amend. Stage by explicit path, run `git status` before each commit, one-line commit messages without attribution.
- Never run `cargo clippy`.
- The code below is `rustfmt`-formatted with the repository's `rustfmt.toml`; transcribe it exactly, so `cargo fmt --all -- --check` stays silent.
- Edits take these forms:
  - "Create `path`" with the whole new file;
  - "Replace the whole contents of `path`";
  - "In `path`, replace" a text which occurs exactly once in the file, "with" its replacement;
  - "insert directly above `#[cfg(test)]`" (the file's one test module) or "directly above the line starting" a unique text;
  - "append inside `mod tests`, directly above its closing brace", which is the last line of the file, after one blank line separating it from the item before;
  - "replace the test module, from `#[cfg(test)]` to the end of the file";
  - "replace everything above `#[cfg(test)]`";
  - "Append to the end of `path`, after one blank line".
- Each inserted or appended block sits between its neighbours with one blank line on each side, as the surrounding code does; code blocks below omit those blank lines at their edges.
- A new file's failing test is the file holding only its `#[cfg(test)] mod tests` block (together with its `pub mod` line); the implementation step then inserts the code above the test module.
- Builds stay warning-free, tests included; a warning is a defect to fix before committing.
- Expected outputs were observed while writing this plan, by executing it end to end in a scratch copy of this branch; test counts and filtered counts assume the tasks are done in order with the code exactly as given, and only the `finished in` timings vary.
- Tasks 24 to 27 test properties of code finished in earlier tasks, so their first run passes; each says so where it runs.

## Decisions taken where the design is silent or contradictory

- Phase 3 built the sending half of the game state wire tree (`From<&Model>` and `protocol::encode_game_state`); this phase adds every receiving conversion, every other wire type, every decode, `PROTOCOL_VERSION`, the limits, the replay format and the protocol tests.
- `PROTOCOL_VERSION` stays 1: no published protocol precedes these wire types.
- Receiving `From` where nothing can fail: protocol.md allows `From` "only for plain enum mirrors, where nothing can fail". Read as every mirror whose every value is a valid model: the enum mirrors (`AbilityPhaseSerialOut` and `RejectionKindSerialOut` included), `AimVectorSerial`, `ScoreSerialOut`, `AppearanceSerial`, `LoadoutSerial`, `RoundStateSerialOut` and `TeamChoiceKindSerialIn`. The design lists `TeamChoiceKindSerialIn` among the `RejectionKind` payloads, but nothing in it can fail; whether the team is one of the game's is the server's check (phase 5).
- Coordinates (protocol.md: "collection lengths and coordinates within `protocol_limits.rs`"): every received point, vector, lattice coordinate and world bound lies within `±2^28` subpixels, which is the inbound cursor limit, 262144 px or 43690 lattice cells; world extents are non-negative. This keeps decoded states clear of the simulation's `i32` arithmetic.
- Collection lengths: a state holds at most `player_cap + 64` members, a bundle at most `MEMBER_LIMIT_HIGHEST` (96) player inputs, a game summary at most 4 team sizes. Spore lists and member events have no limit of their own.
- `validate()` is `GameSettings::validate(&self) -> Result<(), RejectionKind>` in `game_settings.rs` (overview.md: "GameSettings, GameModeKind, validation"). Both `TryFrom<GameSettingsSerialIn>` (a request) and `TryFrom<GameSettingsSerialOut>` (a snapshot or replay header, its rejection turned into an `AppError`) call it. Check order: fields the mode does not have, title, world width and height, player minimum, player cap, team count, leaderboard length, cross-field rules.
- A required field that is missing (srv without a player minimum, skm without a team count) is `SettingOutOfRange { bound: Below }`; the design names no kind for it. A player minimum above 32 is `SettingOutOfRange { field: PlayerMinimum, bound: Above }`, "Player minimum can be at most 32"; the design lists only the lower bound.
- Teams in a state ("team exactly in skm"): a Participant has one of the game's teams exactly in skirmish; a Spectator never has a team.
- Text rules: `normalize_screen_name` and `normalize_title` trim and then check, for request payloads; `check_screen_name` and `check_title` check without trimming, for server output. `normalize_password` treats an empty password as none and does not trim; `PasswordRequired` stays the server's answer.
- A non-increasing `client_tick` (testing.md, semantic validation) is judged against the member's last accepted input inside the game task (server.md#input-validation-and-speed-clamp), so its test belongs to phase 5.
- Rejection texts are `RejectionKind::message_text` in `shared`; the form field each attaches to is the web menu form engine's (phase 8). `TeamUnbalanced` names teams in lowercase as the original's `config.colors.teams` did, through the new `TeamKind::as_str` and its `TryFrom<&str>`.
- `CloseReasonKind` is a plain enum so the client can map a close code back to a kind (`from_close_code`). `reason_text` is `None` for `InternalError` (no reason) and `ProtocolViolation`, whose short cause (decode, conversion, text frame, oversize, validation) the server names in phase 5. `UNREACHABLE_CLIENT_NOTICE` covers an abnormal close, a failed open and any unknown code.
- `AppError` only: `AppErrorStatic` arrives in phase 5 with its first consumer.
- `GameId` lives in `game_summary.rs` beside `GameSummary` and derives `Hash` for the registry key of phase 5; building a summary from a running game is the game task's (phase 5).
- Replay records are framed by a little-endian `u32` length. `ReplayReadError` adds `Truncated`, `MissingHeader` and `InvalidRecord { record_index, error }` to the design's `ProtocolVersionMismatch`. The record helpers (`get_version_prefix`, `get_header_record_bytes`, `get_bundle_record_bytes`) are public for the incremental capture of phase 5. `run_replay` returns one `TickChecksum` per bundle; `format_tick_checksums` writes phase 3's checksum line format.
- Fixtures: the scripted players move from `determinism.rs` to `shared/tests/replay.rs` (`regenerate_scripted_game_fixture`, as phase 3 announced); the golden-sequence test becomes the generic fixture test of `replay.rs`; the two-run test and the snapshot round trip in `determinism.rs` replay the fixture. `regenerate_scripted_game_checksums` is removed. The committed checksums stay byte-identical, and the fixture is 347198 bytes.
- `protocol_dump` decodes hex and base64 itself rather than through a new crate; `replay --checksums` prints only the checksum lines, which the regeneration script redirects into each `.checksums` file; errors print their causes without minimer's backtraces.
- Model to wire to model "over scripted game states with every ability phase, round phase and projectile kind": every tick of the 10,000-tick fixture, with an assertion that the run reached every phase variant, field centres and both received effects; each wire file also has hand-built unit tests.
- Round trip "for every wire type": every wire type is reachable from a message variant or the replay header, so the codec tests round-trip every message variant and the header.
- Size budgets are measured on the whole message frame, the variant tag included, which is at least the size of the wire type alone.

## File structure

- Create `shared/src/error/mod.rs`: declares `error`.
- Create `shared/src/error/error.rs`: `AppError`.
- Create `shared/src/protocol/protocol_limits.rs`: every protocol limit, the member limit, the screen name, title and password rules.
- Create `shared/src/protocol/rejection.rs`: `RequestKind`, `SettingFieldKind`, `RangeBoundKind`, `RejectionKind`, the rejection texts, their `...SerialOut` mirrors.
- Create `shared/src/protocol/close_reason.rs`: `CloseReasonKind`, close codes, reason texts, client notices.
- Create `shared/src/protocol/input_bundle_serial.rs`: `InputBundleSerialOut`, `PlayerTickInputSerialOut`, `MemberEventSerialOut`, the press-bit conversion.
- Create `shared/src/member/joiner.rs`: `Joiner`, `TeamChoiceKind`.
- Create `shared/src/protocol/message_in.rs`: `MessageSerialIn`, `GameSettingsSerialIn`, `JoinerSerialIn`, `TeamChoiceKindSerialIn`, `PlayerInputSerialIn` and their conversions.
- Create `shared/src/game/player_input.rs`: `PlayerInput`.
- Create `shared/src/game/game_summary.rs`: `GameId`, `GameSummary`.
- Create `shared/src/protocol/lobby.rs`: `GameSummarySerialOut`.
- Create `shared/src/protocol/message_out.rs`: `MessageSerialOut`, `GameSnapshotSerialOut`.
- Create `shared/src/protocol/replay_serial.rs`: `ReplayHeaderSerialOut`.
- Create `shared/src/replay/mod.rs`: declares `replay`.
- Create `shared/src/replay/replay.rs`: `ReplayHeader`, `ReplayLog`, `ReplayReadError`, `TickChecksum`, the record stream, `run_replay`, the checksum lines.
- Create `tools/protocol_dump/src/frame_encoding.rs`: binary, hex and base64 frame input.
- Create `tools/protocol_dump/src/frame_dump.rs`: `DirectionKind`, a frame's `Debug` text.
- Create `tools/protocol_dump/src/replay_dump.rs`: a replay's bundles or checksum lines.
- Create `shared/tests/replay.rs`: the scripted players, the fixture regeneration, the fixture test.
- Create `shared/tests/fixtures/scripted_game.replay`: the scripted game (generated).
- Create `shared/tests/helpers/mod.rs`: declares `replay_fixture`.
- Create `shared/tests/helpers/replay_fixture.rs`: reads the scripted game fixture.
- Create `shared/tests/protocol.rs`: model to wire to model over the scripted game, malformed input, size budgets.
- Modify `shared/src/lib.rs`: declares `error` and `replay`.
- Modify `shared/src/protocol/mod.rs`: declares the new protocol modules.
- Modify `shared/src/member/member.rs`: team names.
- Modify `shared/src/member/mod.rs`: declares `joiner`.
- Modify `shared/src/game/mod.rs`: declares `player_input` and `game_summary`.
- Modify `shared/src/game/game_settings.rs`: `GameSettings::validate`.
- Modify `shared/src/game/test_fixture.rs`: an organism with projectiles.
- Modify `shared/src/protocol/protocol.rs`: `PROTOCOL_VERSION`; message, game state and replay record codecs.
- Modify `shared/src/protocol/geometry_serial.rs`: receiving conversions, `AimVectorSerial`, the coordinate check.
- Modify `shared/src/protocol/organism_serial.rs`: receiving conversions.
- Modify `shared/src/protocol/member_serial.rs`: receiving conversions.
- Modify `shared/src/protocol/game_state_serial.rs`: receiving conversions and the state invariants.
- Modify `shared/tests/determinism.rs`: the two-run test and the snapshot round trip over the fixture.
- Modify `tools/protocol_dump/src/main.rs`: the command line.
- Modify `scripts/test/regenerate-replay-fixtures.sh`: rebuilds the fixture, then every checksum file through `protocol_dump`.

---

### Task 1: Application error type

**Files:**
- Create: `shared/src/error/error.rs`
- Create: `shared/src/error/mod.rs`
- Modify: `shared/src/lib.rs`
- Test: inline `mod tests` in `shared/src/error/error.rs`

Decode and conversion failures return `AppError` (overview.md, `shared` crate). It is minimer's newtype, as in eafora. `AppErrorStatic` waits for its first consumer, the password hashing of phase 5.

- [ ] **Step 1: Write the failing test**

Create `shared/src/error/error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_keeps_the_message() {
        let error: AppError = AppError::new("members are out of order");

        assert_eq!(error.message, "Error: members are out of order");
    }
}
```

Create `shared/src/error/mod.rs`:

```rust
pub mod error;

pub use error::*;
```

In `shared/src/lib.rs`, replace:

```rust
pub mod ability;
```

with:

```rust
pub mod ability;
pub mod error;
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib error::`

Expected: FAIL; the first error reads `` error[E0425]: cannot find type `AppError` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/error/error.rs`, insert directly above `#[cfg(test)]`:

```rust
minimer::define_app_error!(pub AppError);
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib error::`

Expected: PASS, `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 234 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/error/error.rs shared/src/error/mod.rs shared/src/lib.rs
git status
git commit -m "app error"
```

### Task 2: Protocol limits

**Files:**
- Create: `shared/src/protocol/protocol_limits.rs`
- Modify: `shared/src/protocol/mod.rs`
- Test: inline `mod tests` in `shared/src/protocol/protocol_limits.rs`

Every limit of protocol.md#limits as a constant, shared by client validation and the server; the coordinate limits bound every point a `TryFrom` accepts, so a decoded state cannot overflow the simulation's `i32` arithmetic.

- [ ] **Step 1: Write the failing test**

Create `shared/src/protocol/protocol_limits.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_member_limit_adds_the_spectator_limit_to_the_player_cap() {
        assert_eq!(get_member_limit(16), 80);
        assert_eq!(MEMBER_LIMIT_HIGHEST, 96);
    }

    #[test]
    fn coordinate_limits_derive_from_the_subpixel_limit() {
        assert_eq!(COORDINATE_LIMIT_PIXELS, 262_144);
        assert_eq!(LATTICE_COORDINATE_LIMIT, 43_690);
    }
}
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub mod protocol;
```

with:

```rust
pub mod protocol;
pub mod protocol_limits;
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub use protocol::*;
```

with:

```rust
pub use protocol::*;
pub use protocol_limits::*;
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib protocol_limits`

Expected: FAIL; the first error reads `` error[E0425]: cannot find value `MEMBER_LIMIT_HIGHEST` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/protocol/protocol_limits.rs`, insert directly above `#[cfg(test)]`:

```rust
use crate::geometry;

pub const INBOUND_FRAME_BYTE_LIMIT: usize = 4096;
pub const SCREEN_NAME_CHARACTER_LIMIT: usize = 64;
pub const TITLE_CHARACTER_LIMIT: usize = 64;
/// bcrypt reads at most 72 bytes.
pub const PASSWORD_BYTE_LIMIT: usize = 72;
pub const WORLD_SIZE_LOWEST_PIXELS: u32 = 300;
pub const WORLD_SIZE_HIGHEST_PIXELS: u32 = 100_000;
pub const PLAYER_MINIMUM_LOWEST: u8 = 2;
pub const PLAYER_CAP_LOWEST: u8 = 2;
pub const PLAYER_CAP_HIGHEST: u8 = 32;
pub const TEAM_COUNT_LOWEST: u8 = 2;
pub const TEAM_COUNT_HIGHEST: u8 = 4;
pub const LEADERBOARD_LENGTH_LOWEST: u8 = 1;
pub const LEADERBOARD_LENGTH_HIGHEST: u8 = 20;
/// Pure Spectators.
pub const SPECTATOR_LIMIT_PER_GAME: u32 = 64;
pub const MEMBER_LIMIT_HIGHEST: u32 = get_member_limit(PLAYER_CAP_HIGHEST);
pub const GAME_LIMIT_PER_SERVER: u32 = 256;
pub const CONNECTION_LIMIT_PER_SERVER: u32 = 4096;
pub const CONNECTION_LIMIT_PER_IP: u32 = 16;
pub const PASSWORD_FAILURE_LIMIT_PER_IP: u32 = 20;
pub const PASSWORD_FAILURE_WINDOW_SECONDS: u64 = 600;
/// Per axis, either sign.
pub const COORDINATE_LIMIT_SUBPIXELS: i32 = 1 << 28;
pub const COORDINATE_LIMIT_PIXELS: i32 = COORDINATE_LIMIT_SUBPIXELS / geometry::SUBPIXELS_PER_PIXEL;
pub const LATTICE_COORDINATE_LIMIT: i32 = COORDINATE_LIMIT_PIXELS / geometry::CELL_WIDTH_PIXELS;

/// Participants of any status plus Spectators.
pub const fn get_member_limit(player_cap: u8) -> u32 {
    player_cap as u32 + SPECTATOR_LIMIT_PER_GAME
}
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib protocol_limits`

Expected: PASS, `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 235 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/protocol_limits.rs shared/src/protocol/mod.rs
git status
git commit -m "protocol limits"
```

### Task 3: Team names

**Files:**
- Modify: `shared/src/member/member.rs`
- Test: inline `mod tests` in `shared/src/member/member.rs`

The rejection text `TeamUnbalanced` names teams the way the original did (`config.colors.teams`: red, blue, green, pink). `TryFrom<&str>` comes with `as_str()` (`docs/conventions/types.md`).

- [ ] **Step 1: Write the failing test**

In `shared/src/member/member.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn as_str_gives_the_lowercase_team_names() {
        let team_names: Vec<&str> = TEAM_ORDER.iter().map(|team| team.as_str()).collect();

        assert_eq!(team_names, vec!["red", "blue", "green", "pink"]);
    }

    #[test]
    fn try_from_reads_every_team_name() {
        for team in TEAM_ORDER {
            assert_eq!(TeamKind::try_from(team.as_str()), Ok(team));
        }
    }

    #[test]
    fn try_from_rejects_an_unknown_team_name() {
        assert_eq!(
            TeamKind::try_from("Red"),
            Err(UnknownTeamName {
                name: String::from("Red"),
            }),
        );
    }
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib member::member`

Expected: FAIL; the first error reads `` error[E0422]: cannot find struct, variant or union type `UnknownTeamName` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/member/member.rs`, replace:

```rust
pub const TEAM_ORDER: [TeamKind; 4] = [TeamKind::Red, TeamKind::Blue, TeamKind::Green, TeamKind::Pink];
```

with:

```rust
pub const TEAM_ORDER: [TeamKind; 4] = [TeamKind::Red, TeamKind::Blue, TeamKind::Green, TeamKind::Pink];
const RED_TEAM_NAME: &str = "red";
const BLUE_TEAM_NAME: &str = "blue";
const GREEN_TEAM_NAME: &str = "green";
const PINK_TEAM_NAME: &str = "pink";
```

In `shared/src/member/member.rs`, replace:

```rust
            TeamKind::Pink => OrganismColorKind::Petal,
        }
    }
}
```

with:

```rust
            TeamKind::Pink => OrganismColorKind::Petal,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            TeamKind::Red => RED_TEAM_NAME,
            TeamKind::Blue => BLUE_TEAM_NAME,
            TeamKind::Green => GREEN_TEAM_NAME,
            TeamKind::Pink => PINK_TEAM_NAME,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownTeamName {
    pub name: String,
}

impl TryFrom<&str> for TeamKind {
    type Error = UnknownTeamName;

    fn try_from(name: &str) -> Result<TeamKind, UnknownTeamName> {
        match name {
            RED_TEAM_NAME => Ok(TeamKind::Red),
            BLUE_TEAM_NAME => Ok(TeamKind::Blue),
            GREEN_TEAM_NAME => Ok(TeamKind::Green),
            PINK_TEAM_NAME => Ok(TeamKind::Pink),
            _ => Err(UnknownTeamName {
                name: String::from(name),
            }),
        }
    }
}
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib member::member`

Expected: PASS, `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 232 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/member/member.rs
git status
git commit -m "team names"
```

### Task 4: Request and rejection kinds

**Files:**
- Create: `shared/src/protocol/rejection.rs`
- Modify: `shared/src/protocol/mod.rs`
- Test: inline `mod tests` in `shared/src/protocol/rejection.rs`

The request models of protocol.md#server-to-client and every rejection's text. The world size text names both dimensions; every other range text names its field. `SettingFieldKind::lowest` and `highest` are the one source of the ranges, so the texts and `GameSettings::validate` (Task 7) cannot drift apart.

- [ ] **Step 1: Write the failing test**

Create `shared/src/protocol/rejection.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_text_gives_the_text_of_every_rejection() {
        let expected_texts: Vec<(RejectionKind, &str)> = vec![
            (RejectionKind::ScreenNameEmpty, "Screen name cannot be left empty"),
            (
                RejectionKind::ScreenNameTooLong,
                "Screen name can be at most 64 characters",
            ),
            (
                RejectionKind::ScreenNameInvalidCharacter,
                "Screen name cannot contain control characters",
            ),
            (RejectionKind::ScreenNameTaken, "Name matches that of another player"),
            (RejectionKind::TitleEmpty, "Title cannot be left blank"),
            (RejectionKind::TitleTooLong, "Title can be at most 64 characters"),
            (
                RejectionKind::TitleInvalidCharacter,
                "Title cannot contain control characters",
            ),
            (RejectionKind::TitleTaken, "Title matches that of another game"),
            (
                RejectionKind::SettingNotApplicable {
                    field: SettingFieldKind::TeamCount,
                },
                "Setting does not apply to this mode",
            ),
            (
                RejectionKind::PlayerCapBelowMinimum,
                "Player cap cannot be less than player minimum",
            ),
            (
                RejectionKind::PlayerCapBelowTeamCount,
                "Player cap cannot be less than the number of teams",
            ),
            (RejectionKind::PasswordRequired, "A password is required for this game"),
            (RejectionKind::PasswordIncorrect, "Password is invalid"),
            (RejectionKind::PasswordTooLong, "Password can be at most 72 bytes"),
            (RejectionKind::GameNotFound, "The game has closed"),
            (RejectionKind::GameFull, "Game is at maximum player capacity"),
            (
                RejectionKind::SpectatorLimitReached,
                "Game is at maximum spectator capacity",
            ),
            (
                RejectionKind::TeamUnbalanced {
                    requested: TeamKind::Red,
                    smaller: TeamKind::Green,
                },
                "Cannot join red team because it already has more players than green",
            ),
            (RejectionKind::RoundInProgress, "Wait for the round to complete"),
            (RejectionKind::AlreadyInGame, "Request is not valid right now"),
            (RejectionKind::NotInGame, "Request is not valid right now"),
            (RejectionKind::NotDead, "Request is not valid right now"),
            (RejectionKind::ServerFull, "The server is at maximum game capacity"),
            (RejectionKind::ServerBusy, "The server is busy; try again"),
            (RejectionKind::RateLimited, "Too many requests; try again"),
        ];

        for (rejection_kind, expected_text) in expected_texts {
            assert_eq!(rejection_kind.message_text(), expected_text);
        }
    }

    #[test]
    fn message_text_names_the_limit_of_an_out_of_range_setting() {
        let expected_texts: Vec<(SettingFieldKind, RangeBoundKind, &str)> = vec![
            (
                SettingFieldKind::WorldSize,
                RangeBoundKind::Below,
                "Dimensions must be at least 300 x 300 px",
            ),
            (
                SettingFieldKind::WorldSize,
                RangeBoundKind::Above,
                "Dimensions can be at most 100000 x 100000 px",
            ),
            (
                SettingFieldKind::PlayerMinimum,
                RangeBoundKind::Below,
                "Player minimum must be at least 2",
            ),
            (
                SettingFieldKind::PlayerMinimum,
                RangeBoundKind::Above,
                "Player minimum can be at most 32",
            ),
            (
                SettingFieldKind::PlayerCap,
                RangeBoundKind::Below,
                "Player cap must be at least 2",
            ),
            (
                SettingFieldKind::PlayerCap,
                RangeBoundKind::Above,
                "Player cap can be at most 32",
            ),
            (
                SettingFieldKind::TeamCount,
                RangeBoundKind::Below,
                "Team count must be at least 2",
            ),
            (
                SettingFieldKind::TeamCount,
                RangeBoundKind::Above,
                "Team count can be at most 4",
            ),
            (
                SettingFieldKind::LeaderboardLength,
                RangeBoundKind::Below,
                "Leaderboard length must be at least 1",
            ),
            (
                SettingFieldKind::LeaderboardLength,
                RangeBoundKind::Above,
                "Leaderboard length can be at most 20",
            ),
        ];

        for (field, bound, expected_text) in expected_texts {
            assert_eq!(
                RejectionKind::SettingOutOfRange { field, bound }.message_text(),
                expected_text
            );
        }
    }

    #[test]
    fn to_app_error_names_the_rejection() {
        let error: AppError = RejectionKind::ScreenNameTooLong.to_app_error();

        assert_eq!(error.message, "Error: rejected: ScreenNameTooLong");
    }
}
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub mod protocol_limits;
```

with:

```rust
pub mod protocol_limits;
pub mod rejection;
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub use protocol_limits::*;
```

with:

```rust
pub use protocol_limits::*;
pub use rejection::*;
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib protocol::rejection`

Expected: FAIL; the first error reads `` error[E0425]: cannot find type `RejectionKind` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/protocol/rejection.rs`, insert directly above `#[cfg(test)]`:

```rust
use crate::error::AppError;
use crate::member::TeamKind;
use crate::protocol::protocol_limits;

const GENERAL_FAULT_TEXT: &str = "Request is not valid right now";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestKind {
    SubscribeGameList,
    CreateGame,
    JoinGame,
    SpectateGame,
    Respawn,
    UpdateAppearance,
    LeaveGame,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingFieldKind {
    WorldSize,
    PlayerMinimum,
    PlayerCap,
    TeamCount,
    LeaderboardLength,
}

impl SettingFieldKind {
    pub fn lowest(self) -> u32 {
        match self {
            SettingFieldKind::WorldSize => protocol_limits::WORLD_SIZE_LOWEST_PIXELS,
            SettingFieldKind::PlayerMinimum => u32::from(protocol_limits::PLAYER_MINIMUM_LOWEST),
            SettingFieldKind::PlayerCap => u32::from(protocol_limits::PLAYER_CAP_LOWEST),
            SettingFieldKind::TeamCount => u32::from(protocol_limits::TEAM_COUNT_LOWEST),
            SettingFieldKind::LeaderboardLength => u32::from(protocol_limits::LEADERBOARD_LENGTH_LOWEST),
        }
    }

    pub fn highest(self) -> u32 {
        match self {
            SettingFieldKind::WorldSize => protocol_limits::WORLD_SIZE_HIGHEST_PIXELS,
            SettingFieldKind::PlayerMinimum | SettingFieldKind::PlayerCap => {
                u32::from(protocol_limits::PLAYER_CAP_HIGHEST)
            }
            SettingFieldKind::TeamCount => u32::from(protocol_limits::TEAM_COUNT_HIGHEST),
            SettingFieldKind::LeaderboardLength => u32::from(protocol_limits::LEADERBOARD_LENGTH_HIGHEST),
        }
    }

    fn label(self) -> &'static str {
        match self {
            SettingFieldKind::WorldSize => "Dimensions",
            SettingFieldKind::PlayerMinimum => "Player minimum",
            SettingFieldKind::PlayerCap => "Player cap",
            SettingFieldKind::TeamCount => "Team count",
            SettingFieldKind::LeaderboardLength => "Leaderboard length",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RangeBoundKind {
    Below,
    Above,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectionKind {
    ScreenNameEmpty,
    ScreenNameTooLong,
    ScreenNameInvalidCharacter,
    ScreenNameTaken,
    TitleEmpty,
    TitleTooLong,
    TitleInvalidCharacter,
    TitleTaken,
    SettingOutOfRange {
        field: SettingFieldKind,
        bound: RangeBoundKind,
    },
    SettingNotApplicable {
        field: SettingFieldKind,
    },
    PlayerCapBelowMinimum,
    PlayerCapBelowTeamCount,
    PasswordRequired,
    PasswordIncorrect,
    PasswordTooLong,
    GameNotFound,
    GameFull,
    SpectatorLimitReached,
    TeamUnbalanced {
        requested: TeamKind,
        smaller: TeamKind,
    },
    RoundInProgress,
    AlreadyInGame,
    NotInGame,
    NotDead,
    ServerFull,
    ServerBusy,
    RateLimited,
}

impl RejectionKind {
    pub fn message_text(self) -> String {
        match self {
            RejectionKind::ScreenNameEmpty => String::from("Screen name cannot be left empty"),
            RejectionKind::ScreenNameTooLong => format!(
                "Screen name can be at most {} characters",
                protocol_limits::SCREEN_NAME_CHARACTER_LIMIT,
            ),
            RejectionKind::ScreenNameInvalidCharacter => String::from("Screen name cannot contain control characters"),
            RejectionKind::ScreenNameTaken => String::from("Name matches that of another player"),
            RejectionKind::TitleEmpty => String::from("Title cannot be left blank"),
            RejectionKind::TitleTooLong => {
                format!(
                    "Title can be at most {} characters",
                    protocol_limits::TITLE_CHARACTER_LIMIT
                )
            }
            RejectionKind::TitleInvalidCharacter => String::from("Title cannot contain control characters"),
            RejectionKind::TitleTaken => String::from("Title matches that of another game"),
            RejectionKind::SettingOutOfRange { field, bound } => get_out_of_range_text(field, bound),
            RejectionKind::SettingNotApplicable { .. } => String::from("Setting does not apply to this mode"),
            RejectionKind::PlayerCapBelowMinimum => String::from("Player cap cannot be less than player minimum"),
            RejectionKind::PlayerCapBelowTeamCount => {
                String::from("Player cap cannot be less than the number of teams")
            }
            RejectionKind::PasswordRequired => String::from("A password is required for this game"),
            RejectionKind::PasswordIncorrect => String::from("Password is invalid"),
            RejectionKind::PasswordTooLong => {
                format!("Password can be at most {} bytes", protocol_limits::PASSWORD_BYTE_LIMIT)
            }
            RejectionKind::GameNotFound => String::from("The game has closed"),
            RejectionKind::GameFull => String::from("Game is at maximum player capacity"),
            RejectionKind::SpectatorLimitReached => String::from("Game is at maximum spectator capacity"),
            RejectionKind::TeamUnbalanced { requested, smaller } => format!(
                "Cannot join {} team because it already has more players than {}",
                requested.as_str(),
                smaller.as_str(),
            ),
            RejectionKind::RoundInProgress => String::from("Wait for the round to complete"),
            RejectionKind::AlreadyInGame | RejectionKind::NotInGame | RejectionKind::NotDead => {
                String::from(GENERAL_FAULT_TEXT)
            }
            RejectionKind::ServerFull => String::from("The server is at maximum game capacity"),
            RejectionKind::ServerBusy => String::from("The server is busy; try again"),
            RejectionKind::RateLimited => String::from("Too many requests; try again"),
        }
    }

    /// For a payload which fails a request rule outside a request, such as a screen name in a snapshot.
    pub fn to_app_error(self) -> AppError {
        AppError::new(&format!("rejected: {self:?}"))
    }
}

fn get_out_of_range_text(field: SettingFieldKind, bound: RangeBoundKind) -> String {
    let (relation, limit): (&str, u32) = match bound {
        RangeBoundKind::Below => ("must be at least", field.lowest()),
        RangeBoundKind::Above => ("can be at most", field.highest()),
    };
    let limit_text: String = match field {
        SettingFieldKind::WorldSize => format!("{limit} x {limit} px"),
        SettingFieldKind::PlayerMinimum
        | SettingFieldKind::PlayerCap
        | SettingFieldKind::TeamCount
        | SettingFieldKind::LeaderboardLength => limit.to_string(),
    };

    format!("{} {relation} {limit_text}", field.label())
}
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib protocol::rejection`

Expected: PASS, `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 240 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/rejection.rs shared/src/protocol/mod.rs
git status
git commit -m "rejection kinds"
```

### Task 5: Rejection wire types

**Files:**
- Modify: `shared/src/protocol/rejection.rs`
- Modify: `shared/src/protocol/member_serial.rs`
- Test: inline `mod tests` in `shared/src/protocol/rejection.rs`

Each request and rejection model has a `...SerialOut` mirror of the same variants (`TeamKindSerial` in place of `TeamKind`). Plain enum mirrors convert with `From` both ways. The receiving `From<TeamKindSerial> for TeamKind` is added beside its sending conversion in `member_serial.rs`.

- [ ] **Step 1: Write the failing test**

In `shared/src/protocol/rejection.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    fn get_every_rejection_kind() -> Vec<RejectionKind> {
        let fields: [SettingFieldKind; 5] = [
            SettingFieldKind::WorldSize,
            SettingFieldKind::PlayerMinimum,
            SettingFieldKind::PlayerCap,
            SettingFieldKind::TeamCount,
            SettingFieldKind::LeaderboardLength,
        ];
        let mut rejection_kinds: Vec<RejectionKind> = vec![
            RejectionKind::ScreenNameEmpty,
            RejectionKind::ScreenNameTooLong,
            RejectionKind::ScreenNameInvalidCharacter,
            RejectionKind::ScreenNameTaken,
            RejectionKind::TitleEmpty,
            RejectionKind::TitleTooLong,
            RejectionKind::TitleInvalidCharacter,
            RejectionKind::TitleTaken,
            RejectionKind::PlayerCapBelowMinimum,
            RejectionKind::PlayerCapBelowTeamCount,
            RejectionKind::PasswordRequired,
            RejectionKind::PasswordIncorrect,
            RejectionKind::PasswordTooLong,
            RejectionKind::GameNotFound,
            RejectionKind::GameFull,
            RejectionKind::SpectatorLimitReached,
            RejectionKind::TeamUnbalanced {
                requested: TeamKind::Pink,
                smaller: TeamKind::Blue,
            },
            RejectionKind::RoundInProgress,
            RejectionKind::AlreadyInGame,
            RejectionKind::NotInGame,
            RejectionKind::NotDead,
            RejectionKind::ServerFull,
            RejectionKind::ServerBusy,
            RejectionKind::RateLimited,
        ];

        for field in fields {
            rejection_kinds.push(RejectionKind::SettingNotApplicable { field });

            for bound in [RangeBoundKind::Below, RangeBoundKind::Above] {
                rejection_kinds.push(RejectionKind::SettingOutOfRange { field, bound });
            }
        }

        rejection_kinds
    }

    #[test]
    fn rejection_kind_serial_out_converts_back_to_every_rejection() {
        for rejection_kind in get_every_rejection_kind() {
            assert_eq!(
                RejectionKind::from(RejectionKindSerialOut::from(&rejection_kind)),
                rejection_kind
            );
        }
    }

    #[test]
    fn request_kind_serial_out_converts_back_to_every_request() {
        let requests: [RequestKind; 7] = [
            RequestKind::SubscribeGameList,
            RequestKind::CreateGame,
            RequestKind::JoinGame,
            RequestKind::SpectateGame,
            RequestKind::Respawn,
            RequestKind::UpdateAppearance,
            RequestKind::LeaveGame,
        ];

        for request in requests {
            assert_eq!(RequestKind::from(RequestKindSerialOut::from(&request)), request);
        }
    }
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib protocol::rejection`

Expected: FAIL; the first error reads `` error[E0433]: cannot find type `RejectionKindSerialOut` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/protocol/rejection.rs`, replace:

```rust
use crate::error::AppError;
use crate::member::TeamKind;
use crate::protocol::protocol_limits;
```

with:

```rust
use bitcode::{Decode, Encode};

use crate::error::AppError;
use crate::member::TeamKind;
use crate::protocol::TeamKindSerial;
use crate::protocol::protocol_limits;
```

In `shared/src/protocol/rejection.rs`, insert directly above the line starting `fn get_out_of_range_text(`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum RequestKindSerialOut {
    SubscribeGameList,
    CreateGame,
    JoinGame,
    SpectateGame,
    Respawn,
    UpdateAppearance,
    LeaveGame,
}

impl From<&RequestKind> for RequestKindSerialOut {
    fn from(request: &RequestKind) -> RequestKindSerialOut {
        match request {
            RequestKind::SubscribeGameList => RequestKindSerialOut::SubscribeGameList,
            RequestKind::CreateGame => RequestKindSerialOut::CreateGame,
            RequestKind::JoinGame => RequestKindSerialOut::JoinGame,
            RequestKind::SpectateGame => RequestKindSerialOut::SpectateGame,
            RequestKind::Respawn => RequestKindSerialOut::Respawn,
            RequestKind::UpdateAppearance => RequestKindSerialOut::UpdateAppearance,
            RequestKind::LeaveGame => RequestKindSerialOut::LeaveGame,
        }
    }
}

impl From<RequestKindSerialOut> for RequestKind {
    fn from(request_serial_out: RequestKindSerialOut) -> RequestKind {
        match request_serial_out {
            RequestKindSerialOut::SubscribeGameList => RequestKind::SubscribeGameList,
            RequestKindSerialOut::CreateGame => RequestKind::CreateGame,
            RequestKindSerialOut::JoinGame => RequestKind::JoinGame,
            RequestKindSerialOut::SpectateGame => RequestKind::SpectateGame,
            RequestKindSerialOut::Respawn => RequestKind::Respawn,
            RequestKindSerialOut::UpdateAppearance => RequestKind::UpdateAppearance,
            RequestKindSerialOut::LeaveGame => RequestKind::LeaveGame,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SettingFieldKindSerialOut {
    WorldSize,
    PlayerMinimum,
    PlayerCap,
    TeamCount,
    LeaderboardLength,
}

impl From<&SettingFieldKind> for SettingFieldKindSerialOut {
    fn from(field: &SettingFieldKind) -> SettingFieldKindSerialOut {
        match field {
            SettingFieldKind::WorldSize => SettingFieldKindSerialOut::WorldSize,
            SettingFieldKind::PlayerMinimum => SettingFieldKindSerialOut::PlayerMinimum,
            SettingFieldKind::PlayerCap => SettingFieldKindSerialOut::PlayerCap,
            SettingFieldKind::TeamCount => SettingFieldKindSerialOut::TeamCount,
            SettingFieldKind::LeaderboardLength => SettingFieldKindSerialOut::LeaderboardLength,
        }
    }
}

impl From<SettingFieldKindSerialOut> for SettingFieldKind {
    fn from(field_serial_out: SettingFieldKindSerialOut) -> SettingFieldKind {
        match field_serial_out {
            SettingFieldKindSerialOut::WorldSize => SettingFieldKind::WorldSize,
            SettingFieldKindSerialOut::PlayerMinimum => SettingFieldKind::PlayerMinimum,
            SettingFieldKindSerialOut::PlayerCap => SettingFieldKind::PlayerCap,
            SettingFieldKindSerialOut::TeamCount => SettingFieldKind::TeamCount,
            SettingFieldKindSerialOut::LeaderboardLength => SettingFieldKind::LeaderboardLength,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum RangeBoundKindSerialOut {
    Below,
    Above,
}

impl From<&RangeBoundKind> for RangeBoundKindSerialOut {
    fn from(bound: &RangeBoundKind) -> RangeBoundKindSerialOut {
        match bound {
            RangeBoundKind::Below => RangeBoundKindSerialOut::Below,
            RangeBoundKind::Above => RangeBoundKindSerialOut::Above,
        }
    }
}

impl From<RangeBoundKindSerialOut> for RangeBoundKind {
    fn from(bound_serial_out: RangeBoundKindSerialOut) -> RangeBoundKind {
        match bound_serial_out {
            RangeBoundKindSerialOut::Below => RangeBoundKind::Below,
            RangeBoundKindSerialOut::Above => RangeBoundKind::Above,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum RejectionKindSerialOut {
    ScreenNameEmpty,
    ScreenNameTooLong,
    ScreenNameInvalidCharacter,
    ScreenNameTaken,
    TitleEmpty,
    TitleTooLong,
    TitleInvalidCharacter,
    TitleTaken,
    SettingOutOfRange {
        field: SettingFieldKindSerialOut,
        bound: RangeBoundKindSerialOut,
    },
    SettingNotApplicable {
        field: SettingFieldKindSerialOut,
    },
    PlayerCapBelowMinimum,
    PlayerCapBelowTeamCount,
    PasswordRequired,
    PasswordIncorrect,
    PasswordTooLong,
    GameNotFound,
    GameFull,
    SpectatorLimitReached,
    TeamUnbalanced {
        requested: TeamKindSerial,
        smaller: TeamKindSerial,
    },
    RoundInProgress,
    AlreadyInGame,
    NotInGame,
    NotDead,
    ServerFull,
    ServerBusy,
    RateLimited,
}

impl From<&RejectionKind> for RejectionKindSerialOut {
    fn from(rejection_kind: &RejectionKind) -> RejectionKindSerialOut {
        match rejection_kind {
            RejectionKind::ScreenNameEmpty => RejectionKindSerialOut::ScreenNameEmpty,
            RejectionKind::ScreenNameTooLong => RejectionKindSerialOut::ScreenNameTooLong,
            RejectionKind::ScreenNameInvalidCharacter => RejectionKindSerialOut::ScreenNameInvalidCharacter,
            RejectionKind::ScreenNameTaken => RejectionKindSerialOut::ScreenNameTaken,
            RejectionKind::TitleEmpty => RejectionKindSerialOut::TitleEmpty,
            RejectionKind::TitleTooLong => RejectionKindSerialOut::TitleTooLong,
            RejectionKind::TitleInvalidCharacter => RejectionKindSerialOut::TitleInvalidCharacter,
            RejectionKind::TitleTaken => RejectionKindSerialOut::TitleTaken,
            RejectionKind::SettingOutOfRange { field, bound } => RejectionKindSerialOut::SettingOutOfRange {
                field: SettingFieldKindSerialOut::from(field),
                bound: RangeBoundKindSerialOut::from(bound),
            },
            RejectionKind::SettingNotApplicable { field } => RejectionKindSerialOut::SettingNotApplicable {
                field: SettingFieldKindSerialOut::from(field),
            },
            RejectionKind::PlayerCapBelowMinimum => RejectionKindSerialOut::PlayerCapBelowMinimum,
            RejectionKind::PlayerCapBelowTeamCount => RejectionKindSerialOut::PlayerCapBelowTeamCount,
            RejectionKind::PasswordRequired => RejectionKindSerialOut::PasswordRequired,
            RejectionKind::PasswordIncorrect => RejectionKindSerialOut::PasswordIncorrect,
            RejectionKind::PasswordTooLong => RejectionKindSerialOut::PasswordTooLong,
            RejectionKind::GameNotFound => RejectionKindSerialOut::GameNotFound,
            RejectionKind::GameFull => RejectionKindSerialOut::GameFull,
            RejectionKind::SpectatorLimitReached => RejectionKindSerialOut::SpectatorLimitReached,
            RejectionKind::TeamUnbalanced { requested, smaller } => RejectionKindSerialOut::TeamUnbalanced {
                requested: TeamKindSerial::from(requested),
                smaller: TeamKindSerial::from(smaller),
            },
            RejectionKind::RoundInProgress => RejectionKindSerialOut::RoundInProgress,
            RejectionKind::AlreadyInGame => RejectionKindSerialOut::AlreadyInGame,
            RejectionKind::NotInGame => RejectionKindSerialOut::NotInGame,
            RejectionKind::NotDead => RejectionKindSerialOut::NotDead,
            RejectionKind::ServerFull => RejectionKindSerialOut::ServerFull,
            RejectionKind::ServerBusy => RejectionKindSerialOut::ServerBusy,
            RejectionKind::RateLimited => RejectionKindSerialOut::RateLimited,
        }
    }
}

impl From<RejectionKindSerialOut> for RejectionKind {
    fn from(rejection_kind_serial_out: RejectionKindSerialOut) -> RejectionKind {
        match rejection_kind_serial_out {
            RejectionKindSerialOut::ScreenNameEmpty => RejectionKind::ScreenNameEmpty,
            RejectionKindSerialOut::ScreenNameTooLong => RejectionKind::ScreenNameTooLong,
            RejectionKindSerialOut::ScreenNameInvalidCharacter => RejectionKind::ScreenNameInvalidCharacter,
            RejectionKindSerialOut::ScreenNameTaken => RejectionKind::ScreenNameTaken,
            RejectionKindSerialOut::TitleEmpty => RejectionKind::TitleEmpty,
            RejectionKindSerialOut::TitleTooLong => RejectionKind::TitleTooLong,
            RejectionKindSerialOut::TitleInvalidCharacter => RejectionKind::TitleInvalidCharacter,
            RejectionKindSerialOut::TitleTaken => RejectionKind::TitleTaken,
            RejectionKindSerialOut::SettingOutOfRange { field, bound } => RejectionKind::SettingOutOfRange {
                field: SettingFieldKind::from(field),
                bound: RangeBoundKind::from(bound),
            },
            RejectionKindSerialOut::SettingNotApplicable { field } => RejectionKind::SettingNotApplicable {
                field: SettingFieldKind::from(field),
            },
            RejectionKindSerialOut::PlayerCapBelowMinimum => RejectionKind::PlayerCapBelowMinimum,
            RejectionKindSerialOut::PlayerCapBelowTeamCount => RejectionKind::PlayerCapBelowTeamCount,
            RejectionKindSerialOut::PasswordRequired => RejectionKind::PasswordRequired,
            RejectionKindSerialOut::PasswordIncorrect => RejectionKind::PasswordIncorrect,
            RejectionKindSerialOut::PasswordTooLong => RejectionKind::PasswordTooLong,
            RejectionKindSerialOut::GameNotFound => RejectionKind::GameNotFound,
            RejectionKindSerialOut::GameFull => RejectionKind::GameFull,
            RejectionKindSerialOut::SpectatorLimitReached => RejectionKind::SpectatorLimitReached,
            RejectionKindSerialOut::TeamUnbalanced { requested, smaller } => RejectionKind::TeamUnbalanced {
                requested: TeamKind::from(requested),
                smaller: TeamKind::from(smaller),
            },
            RejectionKindSerialOut::RoundInProgress => RejectionKind::RoundInProgress,
            RejectionKindSerialOut::AlreadyInGame => RejectionKind::AlreadyInGame,
            RejectionKindSerialOut::NotInGame => RejectionKind::NotInGame,
            RejectionKindSerialOut::NotDead => RejectionKind::NotDead,
            RejectionKindSerialOut::ServerFull => RejectionKind::ServerFull,
            RejectionKindSerialOut::ServerBusy => RejectionKind::ServerBusy,
            RejectionKindSerialOut::RateLimited => RejectionKind::RateLimited,
        }
    }
}
```

In `shared/src/protocol/member_serial.rs`, insert directly above `#[cfg(test)]`:

```rust
impl From<TeamKindSerial> for TeamKind {
    fn from(team_serial: TeamKindSerial) -> TeamKind {
        match team_serial {
            TeamKindSerial::Red => TeamKind::Red,
            TeamKindSerial::Blue => TeamKind::Blue,
            TeamKindSerial::Green => TeamKind::Green,
            TeamKindSerial::Pink => TeamKind::Pink,
        }
    }
}
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib protocol::rejection`

Expected: PASS, `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 240 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/rejection.rs shared/src/protocol/member_serial.rs
git status
git commit -m "rejection wire types"
```

### Task 6: Screen name, title and password rules

**Files:**
- Modify: `shared/src/protocol/protocol_limits.rs`
- Test: inline `mod tests` in `shared/src/protocol/protocol_limits.rs`

Names and titles are trimmed, 1 to 64 characters, without control characters; passwords are at most 72 bytes and not trimmed, and an empty password means none (protocol.md#limits). The `normalize_` functions serve request payloads; the `check_` functions serve server output, which is already trimmed.

- [ ] **Step 1: Write the failing test**

In `shared/src/protocol/protocol_limits.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn normalize_screen_name_trims_and_accepts_the_character_limit() {
        let longest_screen_name: String = "é".repeat(64);

        assert_eq!(normalize_screen_name("  Blob  "), Ok(String::from("Blob")));
        assert_eq!(
            normalize_screen_name(&longest_screen_name),
            Ok(longest_screen_name.clone())
        );
    }

    #[test]
    fn normalize_screen_name_rejects_empty_long_and_control_text() {
        assert_eq!(normalize_screen_name(" \t "), Err(RejectionKind::ScreenNameEmpty));
        assert_eq!(
            normalize_screen_name(&"a".repeat(65)),
            Err(RejectionKind::ScreenNameTooLong)
        );
        assert_eq!(
            normalize_screen_name("Bl\u{7}ob"),
            Err(RejectionKind::ScreenNameInvalidCharacter)
        );
    }

    #[test]
    fn check_screen_name_does_not_trim() {
        assert_eq!(check_screen_name(" "), Ok(()));
        assert_eq!(check_screen_name(""), Err(RejectionKind::ScreenNameEmpty));
    }

    #[test]
    fn normalize_title_trims_and_checks_the_title_rules() {
        assert_eq!(normalize_title(" Arena "), Ok(String::from("Arena")));
        assert_eq!(normalize_title(&"t".repeat(64)), Ok("t".repeat(64)));
        assert_eq!(normalize_title(""), Err(RejectionKind::TitleEmpty));
        assert_eq!(normalize_title(&"t".repeat(65)), Err(RejectionKind::TitleTooLong));
        assert_eq!(normalize_title("Are\nna"), Err(RejectionKind::TitleInvalidCharacter));
    }

    #[test]
    fn normalize_password_treats_empty_as_none_and_limits_bytes() {
        let longest_password: String = "p".repeat(72);

        assert_eq!(normalize_password(None), Ok(None));
        assert_eq!(normalize_password(Some(String::new())), Ok(None));
        assert_eq!(
            normalize_password(Some(String::from(" pass "))),
            Ok(Some(String::from(" pass ")))
        );
        assert_eq!(
            normalize_password(Some(longest_password.clone())),
            Ok(Some(longest_password))
        );
        assert_eq!(
            normalize_password(Some("é".repeat(37))),
            Err(RejectionKind::PasswordTooLong)
        );
    }
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib protocol_limits`

Expected: FAIL; the first error reads `` error[E0433]: cannot find type `RejectionKind` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/protocol/protocol_limits.rs`, replace:

```rust
use crate::geometry;
```

with:

```rust
use crate::geometry;
use crate::protocol::RejectionKind;
```

In `shared/src/protocol/protocol_limits.rs`, replace:

```rust

/// Participants of any status plus Spectators.
```

with:

```rust
const SCREEN_NAME_REJECTION_KINDS: TextRejectionKinds = TextRejectionKinds {
    empty: RejectionKind::ScreenNameEmpty,
    too_long: RejectionKind::ScreenNameTooLong,
    invalid_character: RejectionKind::ScreenNameInvalidCharacter,
};
const TITLE_REJECTION_KINDS: TextRejectionKinds = TextRejectionKinds {
    empty: RejectionKind::TitleEmpty,
    too_long: RejectionKind::TitleTooLong,
    invalid_character: RejectionKind::TitleInvalidCharacter,
};

struct TextRejectionKinds {
    empty: RejectionKind,
    too_long: RejectionKind,
    invalid_character: RejectionKind,
}

/// Participants of any status plus Spectators.
```

In `shared/src/protocol/protocol_limits.rs`, insert directly above `#[cfg(test)]`:

```rust
/// The trimmed screen name.
pub fn normalize_screen_name(screen_name: &str) -> Result<String, RejectionKind> {
    let trimmed_screen_name: &str = screen_name.trim();
    check_screen_name(trimmed_screen_name)?;

    Ok(String::from(trimmed_screen_name))
}

pub fn check_screen_name(screen_name: &str) -> Result<(), RejectionKind> {
    check_text(screen_name, SCREEN_NAME_CHARACTER_LIMIT, &SCREEN_NAME_REJECTION_KINDS)
}

/// The trimmed title.
pub fn normalize_title(title: &str) -> Result<String, RejectionKind> {
    let trimmed_title: &str = title.trim();
    check_title(trimmed_title)?;

    Ok(String::from(trimmed_title))
}

pub fn check_title(title: &str) -> Result<(), RejectionKind> {
    check_text(title, TITLE_CHARACTER_LIMIT, &TITLE_REJECTION_KINDS)
}

/// An empty password means none; passwords are not trimmed.
pub fn normalize_password(password: Option<String>) -> Result<Option<String>, RejectionKind> {
    let Some(password): Option<String> = password.filter(|password| !password.is_empty()) else {
        return Ok(None);
    };

    if password.len() > PASSWORD_BYTE_LIMIT {
        return Err(RejectionKind::PasswordTooLong);
    }

    Ok(Some(password))
}

fn check_text(text: &str, character_limit: usize, rejection_kinds: &TextRejectionKinds) -> Result<(), RejectionKind> {
    if text.is_empty() {
        return Err(rejection_kinds.empty);
    }

    if text.chars().count() > character_limit {
        return Err(rejection_kinds.too_long);
    }

    if text.chars().any(char::is_control) {
        return Err(rejection_kinds.invalid_character);
    }

    Ok(())
}
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib protocol_limits`

Expected: PASS, `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 243 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/protocol_limits.rs
git status
git commit -m "text limits"
```

### Task 7: Game settings validation

**Files:**
- Modify: `shared/src/game/game_settings.rs`
- Test: inline `mod tests` in `shared/src/game/game_settings.rs`

`GameSettings::validate` is the shared validation of protocol.md#limits: fields the mode lacks are `SettingNotApplicable`, a required field that is missing counts as below its range, then the title, the ranges, and the cross-field rules of the mode's fields only. The first broken rule is returned, in that order.

- [ ] **Step 1: Write the failing test**

In `shared/src/game/game_settings.rs`, replace:

```rust
mod tests {
    use super::*;
```

with:

```rust
mod tests {
    use super::*;
    use crate::game::test_fixture;
```

In `shared/src/game/game_settings.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    fn create_settings(mode: GameModeKind) -> GameSettings {
        test_fixture::create_settings(mode, WorldShapeKind::Rectangle, 800)
    }

    fn get_out_of_range(field: SettingFieldKind, bound: RangeBoundKind) -> Result<(), RejectionKind> {
        Err(RejectionKind::SettingOutOfRange { field, bound })
    }

    #[test]
    fn validate_accepts_the_fixture_settings_of_every_mode() {
        for mode in [GameModeKind::FreeForAll, GameModeKind::Skirmish, GameModeKind::Survival] {
            assert_eq!(create_settings(mode).validate(), Ok(()));
        }
    }

    #[test]
    fn validate_rejects_a_field_the_mode_does_not_have() {
        let mut free_for_all_settings: GameSettings = create_settings(GameModeKind::FreeForAll);
        free_for_all_settings.team_count = Some(2);
        let mut skirmish_settings: GameSettings = create_settings(GameModeKind::Skirmish);
        skirmish_settings.player_minimum = Some(2);
        let mut survival_settings: GameSettings = create_settings(GameModeKind::Survival);
        survival_settings.team_count = Some(2);

        assert_eq!(
            free_for_all_settings.validate(),
            Err(RejectionKind::SettingNotApplicable {
                field: SettingFieldKind::TeamCount,
            }),
        );
        assert_eq!(
            skirmish_settings.validate(),
            Err(RejectionKind::SettingNotApplicable {
                field: SettingFieldKind::PlayerMinimum,
            }),
        );
        assert_eq!(
            survival_settings.validate(),
            Err(RejectionKind::SettingNotApplicable {
                field: SettingFieldKind::TeamCount,
            }),
        );
    }

    #[test]
    fn validate_counts_a_missing_required_field_as_below_range() {
        let mut skirmish_settings: GameSettings = create_settings(GameModeKind::Skirmish);
        skirmish_settings.team_count = None;
        let mut survival_settings: GameSettings = create_settings(GameModeKind::Survival);
        survival_settings.player_minimum = None;

        assert_eq!(
            skirmish_settings.validate(),
            get_out_of_range(SettingFieldKind::TeamCount, RangeBoundKind::Below),
        );
        assert_eq!(
            survival_settings.validate(),
            get_out_of_range(SettingFieldKind::PlayerMinimum, RangeBoundKind::Below),
        );
    }

    #[test]
    fn validate_checks_the_title() {
        let mut settings: GameSettings = create_settings(GameModeKind::FreeForAll);
        settings.title = String::new();

        assert_eq!(settings.validate(), Err(RejectionKind::TitleEmpty));
    }

    #[test]
    fn validate_accepts_world_sizes_at_the_limits_and_rejects_past_them() {
        let mut settings: GameSettings = create_settings(GameModeKind::FreeForAll);

        settings.world_width_pixels = 300;
        settings.world_height_pixels = 100_000;
        assert_eq!(settings.validate(), Ok(()));

        settings.world_width_pixels = 299;
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::WorldSize, RangeBoundKind::Below)
        );

        settings.world_width_pixels = 300;
        settings.world_height_pixels = 100_001;
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::WorldSize, RangeBoundKind::Above)
        );
    }

    #[test]
    fn validate_accepts_counts_at_the_limits_and_rejects_past_them() {
        let mut settings: GameSettings = create_settings(GameModeKind::Survival);
        settings.player_minimum = Some(2);
        settings.player_cap = 32;
        settings.leaderboard_length = 20;
        assert_eq!(settings.validate(), Ok(()));

        settings.player_minimum = Some(1);
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::PlayerMinimum, RangeBoundKind::Below)
        );

        settings.player_minimum = Some(33);
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::PlayerMinimum, RangeBoundKind::Above)
        );

        settings.player_minimum = Some(2);
        settings.player_cap = 33;
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::PlayerCap, RangeBoundKind::Above)
        );

        settings.player_cap = 1;
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::PlayerCap, RangeBoundKind::Below)
        );

        settings.player_cap = 2;
        settings.leaderboard_length = 0;
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::LeaderboardLength, RangeBoundKind::Below),
        );

        settings.leaderboard_length = 21;
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::LeaderboardLength, RangeBoundKind::Above),
        );
    }

    #[test]
    fn validate_accepts_team_counts_at_the_limits_and_rejects_past_them() {
        let mut settings: GameSettings = create_settings(GameModeKind::Skirmish);

        settings.team_count = Some(4);
        assert_eq!(settings.validate(), Ok(()));

        settings.team_count = Some(1);
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::TeamCount, RangeBoundKind::Below)
        );

        settings.team_count = Some(5);
        assert_eq!(
            settings.validate(),
            get_out_of_range(SettingFieldKind::TeamCount, RangeBoundKind::Above)
        );
    }

    #[test]
    fn validate_applies_the_cross_field_rules_of_the_mode_only() {
        let mut survival_settings: GameSettings = create_settings(GameModeKind::Survival);
        survival_settings.player_minimum = Some(5);
        survival_settings.player_cap = 4;
        let mut skirmish_settings: GameSettings = create_settings(GameModeKind::Skirmish);
        skirmish_settings.team_count = Some(4);
        skirmish_settings.player_cap = 3;
        let mut free_for_all_settings: GameSettings = create_settings(GameModeKind::FreeForAll);
        free_for_all_settings.player_cap = 2;

        assert_eq!(survival_settings.validate(), Err(RejectionKind::PlayerCapBelowMinimum));
        assert_eq!(
            skirmish_settings.validate(),
            Err(RejectionKind::PlayerCapBelowTeamCount)
        );
        assert_eq!(free_for_all_settings.validate(), Ok(()));
    }
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib game_settings`

Expected: FAIL; the first error reads `` error[E0425]: cannot find type `SettingFieldKind` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/game/game_settings.rs`, replace:

```rust
use crate::world::WorldShapeKind;
```

with:

```rust
use crate::protocol::protocol_limits;
use crate::protocol::{RangeBoundKind, RejectionKind, SettingFieldKind};
use crate::world::WorldShapeKind;
```

In `shared/src/game/game_settings.rs`, replace:

```rust
    pub leaderboard_length: u8,
}
```

with:

```rust
    pub leaderboard_length: u8,
}

impl GameSettings {
    /// The protocol limits and the cross-field rules of the mode.
    pub fn validate(&self) -> Result<(), RejectionKind> {
        self.check_mode_fields()?;
        protocol_limits::check_title(&self.title)?;
        check_setting_range(self.world_width_pixels, SettingFieldKind::WorldSize)?;
        check_setting_range(self.world_height_pixels, SettingFieldKind::WorldSize)?;

        if let Some(player_minimum) = self.player_minimum {
            check_setting_range(u32::from(player_minimum), SettingFieldKind::PlayerMinimum)?;
        }

        check_setting_range(u32::from(self.player_cap), SettingFieldKind::PlayerCap)?;

        if let Some(team_count) = self.team_count {
            check_setting_range(u32::from(team_count), SettingFieldKind::TeamCount)?;
        }

        check_setting_range(u32::from(self.leaderboard_length), SettingFieldKind::LeaderboardLength)?;

        if self.player_minimum.is_some_and(|player_minimum| self.player_cap < player_minimum) {
            return Err(RejectionKind::PlayerCapBelowMinimum);
        }

        if self.team_count.is_some_and(|team_count| self.player_cap < team_count) {
            return Err(RejectionKind::PlayerCapBelowTeamCount);
        }

        Ok(())
    }

    /// A missing required field counts as below its range.
    fn check_mode_fields(&self) -> Result<(), RejectionKind> {
        let has_player_minimum: bool = self.player_minimum.is_some();
        let has_team_count: bool = self.team_count.is_some();

        match self.mode {
            GameModeKind::FreeForAll if has_player_minimum => Err(get_not_applicable(SettingFieldKind::PlayerMinimum)),
            GameModeKind::FreeForAll if has_team_count => Err(get_not_applicable(SettingFieldKind::TeamCount)),
            GameModeKind::Skirmish if has_player_minimum => Err(get_not_applicable(SettingFieldKind::PlayerMinimum)),
            GameModeKind::Skirmish if !has_team_count => Err(get_below_range(SettingFieldKind::TeamCount)),
            GameModeKind::Survival if has_team_count => Err(get_not_applicable(SettingFieldKind::TeamCount)),
            GameModeKind::Survival if !has_player_minimum => Err(get_below_range(SettingFieldKind::PlayerMinimum)),
            GameModeKind::FreeForAll | GameModeKind::Skirmish | GameModeKind::Survival => Ok(()),
        }
    }
}
```

In `shared/src/game/game_settings.rs`, insert directly above `#[cfg(test)]`:

```rust
fn check_setting_range(value: u32, field: SettingFieldKind) -> Result<(), RejectionKind> {
    if value < field.lowest() {
        return Err(get_below_range(field));
    }

    if value > field.highest() {
        return Err(RejectionKind::SettingOutOfRange {
            field,
            bound: RangeBoundKind::Above,
        });
    }

    Ok(())
}

fn get_below_range(field: SettingFieldKind) -> RejectionKind {
    RejectionKind::SettingOutOfRange {
        field,
        bound: RangeBoundKind::Below,
    }
}

fn get_not_applicable(field: SettingFieldKind) -> RejectionKind {
    RejectionKind::SettingNotApplicable { field }
}
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib game_settings`

Expected: PASS, `test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 247 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/game/game_settings.rs
git status
git commit -m "game settings validation"
```

### Task 8: Close reasons and protocol version

**Files:**
- Create: `shared/src/protocol/close_reason.rs`
- Modify: `shared/src/protocol/protocol.rs`
- Modify: `shared/src/protocol/mod.rs`
- Test: inline `mod tests` in `shared/src/protocol/close_reason.rs`

`CloseReasonKind` carries the close codes, reason texts and client notices of protocol.md#server-to-client, so the server writes and the client reads one table; `PROTOCOL_VERSION` arrives with its first reader, the mismatch reason text.

- [ ] **Step 1: Write the failing test**

Create `shared/src/protocol/close_reason.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_code_gives_the_code_of_every_close_reason() {
        let close_codes: Vec<u16> = CLOSE_REASON_KINDS.iter().map(|close_reason| close_reason.close_code()).collect();

        assert_eq!(close_codes, vec![1001, 1008, 1011, 4001, 4002, 4003, 4004, 4005, 4006]);
    }

    #[test]
    fn from_close_code_reads_every_close_code() {
        for close_reason in CLOSE_REASON_KINDS {
            assert_eq!(
                CloseReasonKind::from_close_code(close_reason.close_code()),
                Some(close_reason)
            );
        }
    }

    #[test]
    fn from_close_code_gives_none_for_an_abnormal_close() {
        assert_eq!(CloseReasonKind::from_close_code(1006), None);
        assert_eq!(CloseReasonKind::from_close_code(1000), None);
    }

    #[test]
    fn reason_text_names_the_protocol_version_on_a_mismatch() {
        assert_eq!(
            CloseReasonKind::ProtocolVersionMismatch.reason_text(),
            Some(String::from("server protocol version 1")),
        );
        assert_eq!(
            CloseReasonKind::SlowConsumer.reason_text(),
            Some(String::from("slow consumer"))
        );
        assert_eq!(CloseReasonKind::InternalError.reason_text(), None);
        assert_eq!(CloseReasonKind::ProtocolViolation.reason_text(), None);
    }

    #[test]
    fn client_notice_gives_the_notice_of_every_close_reason() {
        let client_notices: Vec<&str> =
            CLOSE_REASON_KINDS.iter().map(|close_reason| close_reason.client_notice()).collect();

        assert_eq!(
            client_notices,
            vec![
                "Disconnected from the game server",
                "Disconnected: protocol error",
                "Disconnected from the game server",
                "Bacter has been updated. Reload the page.",
                "Disconnected: the connection could not keep up",
                "Too many incorrect passwords",
                "Too many requests",
                "Disconnected from the game server",
                "The server is full",
            ],
        );
    }
}
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub mod game_state_serial;
```

with:

```rust
pub mod close_reason;
pub mod game_state_serial;
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub use game_state_serial::*;
```

with:

```rust
pub use close_reason::*;
pub use game_state_serial::*;
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib close_reason`

Expected: FAIL; the first error reads `` error[E0425]: cannot find value `CLOSE_REASON_KINDS` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/protocol/close_reason.rs`, insert directly above `#[cfg(test)]`:

```rust
use crate::protocol;

/// For an abnormal close, a failed open, or a close code no close reason uses.
pub const UNREACHABLE_CLIENT_NOTICE: &str = "Cannot reach the game server";
const DISCONNECTED_CLIENT_NOTICE: &str = "Disconnected from the game server";
const CLOSE_REASON_KINDS: [CloseReasonKind; 9] = [
    CloseReasonKind::ServerShutdown,
    CloseReasonKind::ProtocolViolation,
    CloseReasonKind::InternalError,
    CloseReasonKind::ProtocolVersionMismatch,
    CloseReasonKind::SlowConsumer,
    CloseReasonKind::PasswordFailureLimit,
    CloseReasonKind::RateLimitAbuse,
    CloseReasonKind::InboundTimeout,
    CloseReasonKind::ServerFull,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReasonKind {
    ServerShutdown,
    ProtocolViolation,
    InternalError,
    ProtocolVersionMismatch,
    SlowConsumer,
    PasswordFailureLimit,
    RateLimitAbuse,
    InboundTimeout,
    ServerFull,
}

impl CloseReasonKind {
    pub fn close_code(self) -> u16 {
        match self {
            CloseReasonKind::ServerShutdown => 1001,
            CloseReasonKind::ProtocolViolation => 1008,
            CloseReasonKind::InternalError => 1011,
            CloseReasonKind::ProtocolVersionMismatch => 4001,
            CloseReasonKind::SlowConsumer => 4002,
            CloseReasonKind::PasswordFailureLimit => 4003,
            CloseReasonKind::RateLimitAbuse => 4004,
            CloseReasonKind::InboundTimeout => 4005,
            CloseReasonKind::ServerFull => 4006,
        }
    }

    pub fn from_close_code(close_code: u16) -> Option<CloseReasonKind> {
        CLOSE_REASON_KINDS.into_iter().find(|close_reason| close_reason.close_code() == close_code)
    }

    /// `None` for `InternalError`, which has no reason, and `ProtocolViolation`, whose reason names its cause.
    pub fn reason_text(self) -> Option<String> {
        match self {
            CloseReasonKind::ServerShutdown => Some(String::from("server shutting down")),
            CloseReasonKind::ProtocolViolation | CloseReasonKind::InternalError => None,
            CloseReasonKind::ProtocolVersionMismatch => {
                Some(format!("server protocol version {}", protocol::PROTOCOL_VERSION))
            }
            CloseReasonKind::SlowConsumer => Some(String::from("slow consumer")),
            CloseReasonKind::PasswordFailureLimit => Some(String::from("password failure limit")),
            CloseReasonKind::RateLimitAbuse => Some(String::from("rate limit")),
            CloseReasonKind::InboundTimeout => Some(String::from("inbound timeout")),
            CloseReasonKind::ServerFull => Some(String::from("server full")),
        }
    }

    pub fn client_notice(self) -> &'static str {
        match self {
            CloseReasonKind::ServerShutdown | CloseReasonKind::InternalError | CloseReasonKind::InboundTimeout => {
                DISCONNECTED_CLIENT_NOTICE
            }
            CloseReasonKind::ProtocolViolation => "Disconnected: protocol error",
            CloseReasonKind::ProtocolVersionMismatch => "Bacter has been updated. Reload the page.",
            CloseReasonKind::SlowConsumer => "Disconnected: the connection could not keep up",
            CloseReasonKind::PasswordFailureLimit => "Too many incorrect passwords",
            CloseReasonKind::RateLimitAbuse => "Too many requests",
            CloseReasonKind::ServerFull => "The server is full",
        }
    }
}
```

In `shared/src/protocol/protocol.rs`, replace:

```rust
use crate::protocol::GameStateSerialOut;
```

with:

```rust
use crate::protocol::GameStateSerialOut;

/// Bumped on any change to a wire type.
pub const PROTOCOL_VERSION: u16 = 1;
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib close_reason`

Expected: PASS, `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 258 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/close_reason.rs shared/src/protocol/protocol.rs shared/src/protocol/mod.rs
git status
git commit -m "close reasons and protocol version"
```

### Task 9: Geometry wire conversions

**Files:**
- Modify: `shared/src/protocol/geometry_serial.rs`
- Test: inline `mod tests` in `shared/src/protocol/geometry_serial.rs`

Receiving conversions for the geometry wire types. Every coordinate is checked against protocol_limits.rs (`±2^28` subpixels, the same in whole pixels and lattice cells), which is the bound the inbound cursor rule names and keeps decoded states clear of `i32` overflow. `AimVectorSerial` is the one geometry type both directions share; every `i16` pair is a valid aim, so it converts with `From`.

- [ ] **Step 1: Write the failing test**

In `shared/src/protocol/geometry_serial.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn try_from_accepts_coordinates_at_the_limits() {
        assert_eq!(
            WorldPoint::try_from(WorldPointSerialOut {
                x: -262_144,
                y: 262_144
            })
            .unwrap(),
            WorldPoint {
                x: -262_144,
                y: 262_144
            },
        );
        assert_eq!(
            SubpixelPoint::try_from(SubpixelPointSerial {
                x: 268_435_456,
                y: -268_435_456,
            })
            .unwrap(),
            SubpixelPoint {
                x: 268_435_456,
                y: -268_435_456,
            },
        );
        assert_eq!(
            LatticeCoordinate::try_from(LatticeCoordinateSerialOut { i: 43_690, j: -43_690 }).unwrap(),
            LatticeCoordinate { i: 43_690, j: -43_690 },
        );
    }

    #[test]
    fn try_from_rejects_coordinates_past_the_limits() {
        assert!(WorldPoint::try_from(WorldPointSerialOut { x: 262_145, y: 0 }).is_err());
        assert!(SubpixelPoint::try_from(SubpixelPointSerial { x: 0, y: i32::MIN }).is_err());
        assert!(SubpixelVector::try_from(SubpixelVectorSerialOut { x: 268_435_457, y: 0 }).is_err());
        assert!(LatticeCoordinate::try_from(LatticeCoordinateSerialOut { i: 0, j: 43_691 }).is_err());
    }

    #[test]
    fn aim_vector_serial_converts_both_ways() {
        let aim: AimVector = AimVector { x: -300, y: 41 };

        assert_eq!(AimVector::from(AimVectorSerial::from(&aim)), aim);
    }
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib geometry_serial`

Expected: FAIL; the first error reads `` error[E0425]: cannot find type `AimVector` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/protocol/geometry_serial.rs`, replace:

```rust
use bitcode::{Decode, Encode};

use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, WorldPoint};
```

with:

```rust
use bitcode::{Decode, Encode};

use crate::ability::AimVector;
use crate::error::AppError;
use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, WorldPoint};
use crate::protocol::protocol_limits;
```

In `shared/src/protocol/geometry_serial.rs`, replace:

```rust
            y: world_point.y,
        }
    }
}
```

with:

```rust
            y: world_point.y,
        }
    }
}

impl TryFrom<WorldPointSerialOut> for WorldPoint {
    type Error = AppError;

    fn try_from(world_point_serial_out: WorldPointSerialOut) -> Result<WorldPoint, AppError> {
        Ok(WorldPoint {
            x: check_coordinate(world_point_serial_out.x, protocol_limits::COORDINATE_LIMIT_PIXELS)?,
            y: check_coordinate(world_point_serial_out.y, protocol_limits::COORDINATE_LIMIT_PIXELS)?,
        })
    }
}
```

In `shared/src/protocol/geometry_serial.rs`, replace:

```rust
            y: subpixel_point.y,
        }
    }
}
```

with:

```rust
            y: subpixel_point.y,
        }
    }
}

impl TryFrom<SubpixelPointSerial> for SubpixelPoint {
    type Error = AppError;

    fn try_from(subpixel_point_serial: SubpixelPointSerial) -> Result<SubpixelPoint, AppError> {
        Ok(SubpixelPoint {
            x: check_coordinate(subpixel_point_serial.x, protocol_limits::COORDINATE_LIMIT_SUBPIXELS)?,
            y: check_coordinate(subpixel_point_serial.y, protocol_limits::COORDINATE_LIMIT_SUBPIXELS)?,
        })
    }
}
```

In `shared/src/protocol/geometry_serial.rs`, replace:

```rust
            y: subpixel_vector.y,
        }
    }
}
```

with:

```rust
            y: subpixel_vector.y,
        }
    }
}

impl TryFrom<SubpixelVectorSerialOut> for SubpixelVector {
    type Error = AppError;

    fn try_from(subpixel_vector_serial_out: SubpixelVectorSerialOut) -> Result<SubpixelVector, AppError> {
        Ok(SubpixelVector {
            x: check_coordinate(
                subpixel_vector_serial_out.x,
                protocol_limits::COORDINATE_LIMIT_SUBPIXELS,
            )?,
            y: check_coordinate(
                subpixel_vector_serial_out.y,
                protocol_limits::COORDINATE_LIMIT_SUBPIXELS,
            )?,
        })
    }
}
```

In `shared/src/protocol/geometry_serial.rs`, replace:

```rust
            j: lattice_coordinate.j,
        }
    }
}
```

with:

```rust
            j: lattice_coordinate.j,
        }
    }
}

impl TryFrom<LatticeCoordinateSerialOut> for LatticeCoordinate {
    type Error = AppError;

    fn try_from(lattice_coordinate_serial_out: LatticeCoordinateSerialOut) -> Result<LatticeCoordinate, AppError> {
        Ok(LatticeCoordinate {
            i: check_coordinate(
                lattice_coordinate_serial_out.i,
                protocol_limits::LATTICE_COORDINATE_LIMIT,
            )?,
            j: check_coordinate(
                lattice_coordinate_serial_out.j,
                protocol_limits::LATTICE_COORDINATE_LIMIT,
            )?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct AimVectorSerial {
    pub x: i16,
    pub y: i16,
}

impl From<&AimVector> for AimVectorSerial {
    fn from(aim: &AimVector) -> AimVectorSerial {
        AimVectorSerial { x: aim.x, y: aim.y }
    }
}

impl From<AimVectorSerial> for AimVector {
    fn from(aim_serial: AimVectorSerial) -> AimVector {
        AimVector {
            x: aim_serial.x,
            y: aim_serial.y,
        }
    }
}

/// Within `limit` of zero, either sign.
pub fn check_coordinate(coordinate: i32, limit: i32) -> Result<i32, AppError> {
    if coordinate.unsigned_abs() > limit.unsigned_abs() {
        return Err(AppError::new(&format!(
            "coordinate {coordinate} is beyond the limit {limit}"
        )));
    }

    Ok(coordinate)
}
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib geometry_serial`

Expected: PASS, `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 262 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/geometry_serial.rs
git status
git commit -m "geometry wire conversions"
```

### Task 10: Organism wire conversions

**Files:**
- Modify: `shared/src/protocol/organism_serial.rs`
- Modify: `shared/src/game/test_fixture.rs`
- Test: inline `mod tests` in `shared/src/protocol/organism_serial.rs`

Receiving conversions for organisms, cell sets, ability phases and projectiles. `CellOccupancy::from_tight_bitmap` (phase 2) owns the bitmap invariants; the conversion adds the origin's coordinate check and turns `None` into an `AppError`. An empty cell set is rejected: after `step` re-tightens, every organism has a cell. The organism with projectiles moves into `test_fixture` because the codec tests of Task 17 build snapshots from it too.

- [ ] **Step 1: Write the failing test**

In `shared/src/game/test_fixture.rs`, replace:

```rust
use crate::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use crate::game::{GameModeKind, GameSettings, GameState};
use crate::geometry::WorldPoint;
use crate::member::{Appearance, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind};
use crate::organism::Organism;
use crate::world::WorldShapeKind;

pub const FIXTURE_SEED: u64 = 0x5eed;
```

with:

```rust
use crate::ability::{
    AbilityPhase, FirstAbilityKind, Loadout, OrganismAbilities, Projectile, SecondAbilityKind, ShotPhase, SporePhase,
    ThirdAbilityKind,
};
use crate::game::{GameModeKind, GameSettings, GameState, Tick};
use crate::geometry::{SubpixelPoint, SubpixelVector, WorldPoint};
use crate::member::{Appearance, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind};
use crate::organism::Organism;
use crate::world::WorldShapeKind;

pub const FIXTURE_SEED: u64 = 0x5eed;
pub const PROJECTILE: Projectile = Projectile {
    position: SubpixelPoint { x: 5, y: 6 },
    velocity: SubpixelVector { x: -1, y: 2 },
};
```

Append to the end of `shared/src/game/test_fixture.rs`, after one blank line:

```rust
/// One cell, with abilities in Active, Cooling, Flying and Secreting phases.
pub fn create_organism_with_projectiles() -> Organism {
    let mut organism: Organism = Organism::new(WorldPoint { x: 40, y: 50 });
    organism.last_hitter = Some(MemberId(3));
    organism.abilities = OrganismAbilities {
        first: AbilityPhase::Active { ends_at: Tick(9) },
        second: AbilityPhase::Cooling { ready_at: Tick(8) },
        third: AbilityPhase::Ready,
        third_center: Some(WorldPoint { x: 1, y: 1 }),
        spore: SporePhase::Secreting {
            ends_at: Tick(7),
            spores: vec![PROJECTILE],
        },
        shots: [
            ShotPhase::Flying {
                ends_at: Tick(6),
                shot: PROJECTILE,
            },
            ShotPhase::Secreting {
                ends_at: Tick(5),
                center: PROJECTILE.position,
            },
        ],
        compressed_until: Some(Tick(4)),
        frozen_until: None,
    };

    organism
}
```

In `shared/src/protocol/organism_serial.rs`, replace the test module, from `#[cfg(test)]` to the end of the file, with:

```rust
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
```

`shared/src/protocol/organism_serial.rs` keeps its two existing tests; the organism they build now comes from the fixture.

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib organism_serial`

Expected: FAIL; the first error reads `` error[E0422]: cannot find struct, variant or union type `LatticeCoordinate` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/protocol/organism_serial.rs`, replace:

```rust
use bitcode::{Decode, Encode};

use crate::ability::{AbilityPhase, OrganismAbilities, Projectile, ShotPhase, SporePhase};
use crate::organism::{CellOccupancy, Organism};
use crate::protocol::{LatticeCoordinateSerialOut, SubpixelPointSerial, SubpixelVectorSerialOut, WorldPointSerialOut};
```

with:

```rust
use bitcode::{Decode, Encode};

use crate::ability::{AbilityPhase, OrganismAbilities, Projectile, ShotPhase, SporePhase};
use crate::error::AppError;
use crate::game::Tick;
use crate::geometry::{LatticeCoordinate, SubpixelPoint, SubpixelVector, WorldPoint};
use crate::member::MemberId;
use crate::organism::{CellOccupancy, Organism};
use crate::protocol::{LatticeCoordinateSerialOut, SubpixelPointSerial, SubpixelVectorSerialOut, WorldPointSerialOut};
```

In `shared/src/protocol/organism_serial.rs`, replace:

```rust
            abilities: OrganismAbilitiesSerialOut::from(&organism.abilities),
        }
    }
}
```

with:

```rust
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
```

In `shared/src/protocol/organism_serial.rs`, replace:

```rust
            row_bitmap_bytes: cell_occupancy.row_bitmap_bytes().to_vec(),
        }
    }
}
```

with:

```rust
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
```

In `shared/src/protocol/organism_serial.rs`, replace:

```rust
            frozen_until: abilities.frozen_until.map(|frozen_until| frozen_until.0),
        }
    }
}
```

with:

```rust
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
```

In `shared/src/protocol/organism_serial.rs`, replace:

```rust
            AbilityPhase::Cooling { ready_at } => AbilityPhaseSerialOut::Cooling { ready_at: ready_at.0 },
        }
    }
}
```

with:

```rust
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
```

In `shared/src/protocol/organism_serial.rs`, replace:

```rust
            SporePhase::Cooling { ready_at } => SporePhaseSerialOut::Cooling { ready_at: ready_at.0 },
        }
    }
}
```

with:

```rust
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
```

In `shared/src/protocol/organism_serial.rs`, replace:

```rust
            ShotPhase::Cooling { ready_at } => ShotPhaseSerialOut::Cooling { ready_at: ready_at.0 },
        }
    }
}
```

with:

```rust
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
```

In `shared/src/protocol/organism_serial.rs`, replace:

```rust
            velocity: SubpixelVectorSerialOut::from(&projectile.velocity),
        }
    }
}
```

with:

```rust
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
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib organism_serial`

Expected: PASS, `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 264 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/organism_serial.rs shared/src/game/test_fixture.rs
git status
git commit -m "organism wire conversions"
```

### Task 11: Member wire conversions

**Files:**
- Modify: `shared/src/protocol/member_serial.rs`
- Test: inline `mod tests` in `shared/src/protocol/member_serial.rs`

Receiving conversions for members, scores, loadouts and the kind mirrors. A member carries a loadout exactly when it is a Participant, and only a Participant has an organism; the mode-dependent team rule needs the settings and sits in the game state conversion (Task 12). The screen name passes the same rule as a joiner's.

- [ ] **Step 1: Write the failing test**

In `shared/src/protocol/member_serial.rs`, replace:

```rust
    use crate::geometry::WorldPoint;
    use crate::member::MemberId;
```

with:

```rust
    use crate::geometry::WorldPoint;
```

In `shared/src/protocol/member_serial.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn try_from_restores_a_member_with_its_organism() {
        let mut member: Member = test_fixture::create_participant_with_organism(MemberId(7), WorldPoint { x: 1, y: 2 });
        member.team = Some(TeamKind::Green);
        member.score = Score {
            kills: 4,
            deaths: 5,
            wins: 6,
        };

        assert_eq!(Member::try_from(MemberSerialOut::from(&member)).unwrap(), member);
    }

    #[test]
    fn try_from_rejects_a_loadout_which_does_not_match_the_role() {
        let mut spectator: Member = test_fixture::create_participant(MemberId(1));
        spectator.role = MemberRoleKind::Spectator;
        let mut participant: Member = test_fixture::create_participant(MemberId(2));
        participant.loadout = None;

        assert!(Member::try_from(MemberSerialOut::from(&spectator)).is_err());
        assert!(Member::try_from(MemberSerialOut::from(&participant)).is_err());
    }

    #[test]
    fn try_from_rejects_an_organism_for_a_spectator() {
        let mut spectator: Member =
            test_fixture::create_participant_with_organism(MemberId(1), WorldPoint { x: 1, y: 2 });
        spectator.role = MemberRoleKind::Spectator;
        spectator.loadout = None;

        assert!(Member::try_from(MemberSerialOut::from(&spectator)).is_err());
    }

    #[test]
    fn try_from_rejects_an_invalid_screen_name() {
        let mut member_serial_out: MemberSerialOut =
            MemberSerialOut::from(&test_fixture::create_participant(MemberId(1)));
        member_serial_out.screen_name = String::from("tab\there");

        assert!(Member::try_from(member_serial_out).is_err());
    }

    #[test]
    fn appearance_serial_converts_back_to_every_color_and_skin() {
        let colors: [OrganismColorKind; 12] = [
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
        let skins: [SkinKind; 4] = [SkinKind::Grid, SkinKind::Circles, SkinKind::Ghost, SkinKind::None];

        for color in colors {
            for skin in skins {
                let appearance: Appearance = Appearance { color, skin };

                assert_eq!(Appearance::from(AppearanceSerial::from(&appearance)), appearance);
            }
        }
    }

    #[test]
    fn loadout_serial_converts_back_to_every_ability_choice() {
        for first in [FirstAbilityKind::Extend, FirstAbilityKind::Compress] {
            for second in [SecondAbilityKind::Immortality, SecondAbilityKind::Freeze] {
                for third in [ThirdAbilityKind::Neutralize, ThirdAbilityKind::Toxin] {
                    let loadout: Loadout = Loadout {
                        appearance: test_fixture::create_loadout().appearance,
                        first,
                        second,
                        third,
                    };

                    assert_eq!(Loadout::from(LoadoutSerial::from(&loadout)), loadout);
                }
            }
        }
    }
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib member_serial`

Expected: FAIL; the first error reads `` error[E0425]: cannot find function, tuple struct or tuple variant `MemberId` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/protocol/member_serial.rs`, replace:

```rust
use bitcode::{Decode, Encode};

use crate::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use crate::member::{Appearance, Member, MemberRoleKind, OrganismColorKind, Score, SkinKind, TeamKind};
use crate::protocol::OrganismSerialOut;
```

with:

```rust
use bitcode::{Decode, Encode};

use crate::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use crate::error::AppError;
use crate::member::{Appearance, Member, MemberId, MemberRoleKind, OrganismColorKind, Score, SkinKind, TeamKind};
use crate::organism::Organism;
use crate::protocol::protocol_limits;
use crate::protocol::{OrganismSerialOut, RejectionKind};
```

In `shared/src/protocol/member_serial.rs`, replace:

```rust
            organism: member.organism.as_ref().map(OrganismSerialOut::from),
        }
    }
}
```

with:

```rust
            organism: member.organism.as_ref().map(OrganismSerialOut::from),
        }
    }
}

impl TryFrom<MemberSerialOut> for Member {
    type Error = AppError;

    fn try_from(member_serial_out: MemberSerialOut) -> Result<Member, AppError> {
        protocol_limits::check_screen_name(&member_serial_out.screen_name).map_err(RejectionKind::to_app_error)?;

        let role: MemberRoleKind = MemberRoleKind::from(member_serial_out.role);
        let is_participant: bool = role == MemberRoleKind::Participant;

        if member_serial_out.loadout.is_some() != is_participant {
            return Err(AppError::new("a member has a loadout exactly when it is a Participant"));
        }

        if member_serial_out.organism.is_some() && !is_participant {
            return Err(AppError::new("only a Participant has an organism"));
        }

        let organism: Option<Organism> = member_serial_out.organism.map(Organism::try_from).transpose()?;

        Ok(Member {
            member_id: MemberId(member_serial_out.member_id),
            screen_name: member_serial_out.screen_name,
            role,
            loadout: member_serial_out.loadout.map(Loadout::from),
            team: member_serial_out.team.map(TeamKind::from),
            score: Score::from(member_serial_out.score),
            organism,
        })
    }
}
```

In `shared/src/protocol/member_serial.rs`, replace:

```rust
            MemberRoleKind::Spectator => MemberRoleKindSerialOut::Spectator,
        }
    }
}
```

with:

```rust
            MemberRoleKind::Spectator => MemberRoleKindSerialOut::Spectator,
        }
    }
}

impl From<MemberRoleKindSerialOut> for MemberRoleKind {
    fn from(role_serial_out: MemberRoleKindSerialOut) -> MemberRoleKind {
        match role_serial_out {
            MemberRoleKindSerialOut::Participant => MemberRoleKind::Participant,
            MemberRoleKindSerialOut::Spectator => MemberRoleKind::Spectator,
        }
    }
}
```

In `shared/src/protocol/member_serial.rs`, replace:

```rust
            wins: score.wins,
        }
    }
}
```

with:

```rust
            wins: score.wins,
        }
    }
}

impl From<ScoreSerialOut> for Score {
    fn from(score_serial_out: ScoreSerialOut) -> Score {
        Score {
            kills: score_serial_out.kills,
            deaths: score_serial_out.deaths,
            wins: score_serial_out.wins,
        }
    }
}
```

In `shared/src/protocol/member_serial.rs`, replace:

```rust
            third: ThirdAbilityKindSerial::from(&loadout.third),
        }
    }
}
```

with:

```rust
            third: ThirdAbilityKindSerial::from(&loadout.third),
        }
    }
}

impl From<LoadoutSerial> for Loadout {
    fn from(loadout_serial: LoadoutSerial) -> Loadout {
        Loadout {
            appearance: Appearance::from(loadout_serial.appearance),
            first: FirstAbilityKind::from(loadout_serial.first),
            second: SecondAbilityKind::from(loadout_serial.second),
            third: ThirdAbilityKind::from(loadout_serial.third),
        }
    }
}
```

In `shared/src/protocol/member_serial.rs`, replace:

```rust
            FirstAbilityKind::Compress => FirstAbilityKindSerial::Compress,
        }
    }
}
```

with:

```rust
            FirstAbilityKind::Compress => FirstAbilityKindSerial::Compress,
        }
    }
}

impl From<FirstAbilityKindSerial> for FirstAbilityKind {
    fn from(first_serial: FirstAbilityKindSerial) -> FirstAbilityKind {
        match first_serial {
            FirstAbilityKindSerial::Extend => FirstAbilityKind::Extend,
            FirstAbilityKindSerial::Compress => FirstAbilityKind::Compress,
        }
    }
}
```

In `shared/src/protocol/member_serial.rs`, replace:

```rust
            SecondAbilityKind::Freeze => SecondAbilityKindSerial::Freeze,
        }
    }
}
```

with:

```rust
            SecondAbilityKind::Freeze => SecondAbilityKindSerial::Freeze,
        }
    }
}

impl From<SecondAbilityKindSerial> for SecondAbilityKind {
    fn from(second_serial: SecondAbilityKindSerial) -> SecondAbilityKind {
        match second_serial {
            SecondAbilityKindSerial::Immortality => SecondAbilityKind::Immortality,
            SecondAbilityKindSerial::Freeze => SecondAbilityKind::Freeze,
        }
    }
}
```

In `shared/src/protocol/member_serial.rs`, replace:

```rust
            ThirdAbilityKind::Toxin => ThirdAbilityKindSerial::Toxin,
        }
    }
}
```

with:

```rust
            ThirdAbilityKind::Toxin => ThirdAbilityKindSerial::Toxin,
        }
    }
}

impl From<ThirdAbilityKindSerial> for ThirdAbilityKind {
    fn from(third_serial: ThirdAbilityKindSerial) -> ThirdAbilityKind {
        match third_serial {
            ThirdAbilityKindSerial::Neutralize => ThirdAbilityKind::Neutralize,
            ThirdAbilityKindSerial::Toxin => ThirdAbilityKind::Toxin,
        }
    }
}
```

In `shared/src/protocol/member_serial.rs`, replace:

```rust
            skin: SkinKindSerial::from(&appearance.skin),
        }
    }
}
```

with:

```rust
            skin: SkinKindSerial::from(&appearance.skin),
        }
    }
}

impl From<AppearanceSerial> for Appearance {
    fn from(appearance_serial: AppearanceSerial) -> Appearance {
        Appearance {
            color: OrganismColorKind::from(appearance_serial.color),
            skin: SkinKind::from(appearance_serial.skin),
        }
    }
}
```

In `shared/src/protocol/member_serial.rs`, replace:

```rust
            OrganismColorKind::Hot => OrganismColorKindSerial::Hot,
        }
    }
}
```

with:

```rust
            OrganismColorKind::Hot => OrganismColorKindSerial::Hot,
        }
    }
}

impl From<OrganismColorKindSerial> for OrganismColorKind {
    fn from(color_serial: OrganismColorKindSerial) -> OrganismColorKind {
        match color_serial {
            OrganismColorKindSerial::Fire => OrganismColorKind::Fire,
            OrganismColorKindSerial::Camel => OrganismColorKind::Camel,
            OrganismColorKindSerial::Clay => OrganismColorKind::Clay,
            OrganismColorKindSerial::Sun => OrganismColorKind::Sun,
            OrganismColorKindSerial::Leaf => OrganismColorKind::Leaf,
            OrganismColorKindSerial::Lime => OrganismColorKind::Lime,
            OrganismColorKindSerial::Sky => OrganismColorKind::Sky,
            OrganismColorKindSerial::Lake => OrganismColorKind::Lake,
            OrganismColorKindSerial::Ocean => OrganismColorKind::Ocean,
            OrganismColorKindSerial::Royal => OrganismColorKind::Royal,
            OrganismColorKindSerial::Petal => OrganismColorKind::Petal,
            OrganismColorKindSerial::Hot => OrganismColorKind::Hot,
        }
    }
}
```

In `shared/src/protocol/member_serial.rs`, replace:

```rust
            SkinKind::None => SkinKindSerial::None,
        }
    }
}
```

with:

```rust
            SkinKind::None => SkinKindSerial::None,
        }
    }
}

impl From<SkinKindSerial> for SkinKind {
    fn from(skin_serial: SkinKindSerial) -> SkinKind {
        match skin_serial {
            SkinKindSerial::Grid => SkinKind::Grid,
            SkinKindSerial::Circles => SkinKind::Circles,
            SkinKindSerial::Ghost => SkinKind::Ghost,
            SkinKindSerial::None => SkinKind::None,
        }
    }
}
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib member_serial`

Expected: PASS, `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 271 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/member_serial.rs
git status
git commit -m "member wire conversions"
```

### Task 12: Game state wire conversions

**Files:**
- Modify: `shared/src/protocol/game_state_serial.rs`
- Test: inline `mod tests` in `shared/src/protocol/game_state_serial.rs`

`GameState::try_from` checks what decoding cannot (protocol.md#wire-types): valid settings (through `GameSettings::validate`), a round state exactly in survival, members strictly ascending, every id below `next_member_id`, at most the member limit, and a team exactly for a skirmish Participant, one of the game's teams. The design says "team exactly in skm"; a Spectator never chooses a team, so the rule binds Participants.

- [ ] **Step 1: Write the failing test**

In `shared/src/protocol/game_state_serial.rs`, replace:

```rust
    use crate::game::{Tick, test_fixture};
    use crate::member::MemberId;
```

with:

```rust
    use crate::game::test_fixture;
    use crate::geometry::WorldPoint;
```

In `shared/src/protocol/game_state_serial.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    fn create_state_with_members(mode: GameModeKind) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, WorldShapeKind::Rectangle, 800);
        state.tick = Tick(40);
        state.next_member_id = MemberId(6);

        for member_id in [MemberId(1), MemberId(4)] {
            let mut member: Member =
                test_fixture::create_participant_with_organism(member_id, WorldPoint { x: 200, y: 300 });

            if mode == GameModeKind::Skirmish {
                member.team = Some(TeamKind::Blue);
            }

            state.members.insert(member_id, member);
        }

        let mut spectator: Member = test_fixture::create_participant(MemberId(5));
        spectator.role = MemberRoleKind::Spectator;
        spectator.loadout = None;
        state.members.insert(MemberId(5), spectator);

        state
    }

    #[test]
    fn try_from_restores_the_state_of_every_mode() {
        for mode in [GameModeKind::FreeForAll, GameModeKind::Skirmish, GameModeKind::Survival] {
            let state: GameState = create_state_with_members(mode);

            assert_eq!(GameState::try_from(GameStateSerialOut::from(&state)).unwrap(), state);
        }
    }

    #[test]
    fn try_from_rejects_unordered_and_duplicate_members() {
        let state_serial_out: GameStateSerialOut =
            GameStateSerialOut::from(&create_state_with_members(GameModeKind::FreeForAll));
        let mut unordered_state_serial_out: GameStateSerialOut = state_serial_out.clone();
        unordered_state_serial_out.members.swap(0, 1);
        let mut duplicate_state_serial_out: GameStateSerialOut = state_serial_out;
        duplicate_state_serial_out.members[1] = duplicate_state_serial_out.members[0].clone();

        assert!(GameState::try_from(unordered_state_serial_out).is_err());
        assert!(GameState::try_from(duplicate_state_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_a_member_id_not_below_the_next_member_id() {
        let mut state_serial_out: GameStateSerialOut =
            GameStateSerialOut::from(&create_state_with_members(GameModeKind::FreeForAll));
        state_serial_out.next_member_id = 5;

        assert!(GameState::try_from(state_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_an_even_random_number_generator_increment() {
        let mut state_serial_out: GameStateSerialOut =
            GameStateSerialOut::from(&create_state_with_members(GameModeKind::FreeForAll));
        state_serial_out.rng.increment = 2;

        assert!(GameState::try_from(state_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_a_team_inconsistent_with_the_mode() {
        let mut free_for_all_state: GameState = create_state_with_members(GameModeKind::FreeForAll);
        free_for_all_state.members.get_mut(&MemberId(1)).unwrap().team = Some(TeamKind::Red);
        let mut skirmish_state: GameState = create_state_with_members(GameModeKind::Skirmish);
        skirmish_state.members.get_mut(&MemberId(4)).unwrap().team = None;
        let mut outside_team_state: GameState = create_state_with_members(GameModeKind::Skirmish);
        outside_team_state.members.get_mut(&MemberId(4)).unwrap().team = Some(TeamKind::Pink);
        let mut spectator_team_state: GameState = create_state_with_members(GameModeKind::Skirmish);
        spectator_team_state.members.get_mut(&MemberId(5)).unwrap().team = Some(TeamKind::Red);

        for state in [
            free_for_all_state,
            skirmish_state,
            outside_team_state,
            spectator_team_state,
        ] {
            assert!(GameState::try_from(GameStateSerialOut::from(&state)).is_err());
        }
    }

    #[test]
    fn try_from_rejects_a_round_inconsistent_with_the_mode() {
        let mut survival_state: GameState = create_state_with_members(GameModeKind::Survival);
        survival_state.round = None;
        let mut free_for_all_state: GameState = create_state_with_members(GameModeKind::FreeForAll);
        free_for_all_state.round = Some(RoundState {
            phase: RoundPhase::Playing,
            phase_started_at: Tick(3),
        });

        assert!(GameState::try_from(GameStateSerialOut::from(&survival_state)).is_err());
        assert!(GameState::try_from(GameStateSerialOut::from(&free_for_all_state)).is_err());
    }

    #[test]
    fn try_from_rejects_a_loadout_inconsistent_with_the_role() {
        let mut state: GameState = create_state_with_members(GameModeKind::FreeForAll);
        state.members.get_mut(&MemberId(1)).unwrap().loadout = None;

        assert!(GameState::try_from(GameStateSerialOut::from(&state)).is_err());
    }

    #[test]
    fn try_from_rejects_invalid_settings() {
        let mut state_serial_out: GameStateSerialOut =
            GameStateSerialOut::from(&create_state_with_members(GameModeKind::FreeForAll));
        state_serial_out.settings.team_count = Some(2);

        assert!(GameState::try_from(state_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_more_members_than_the_member_limit() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800);
        let member_limit: u32 = protocol_limits::get_member_limit(state.settings.player_cap);

        for member_index in 0..=member_limit {
            state.members.insert(
                MemberId(member_index),
                test_fixture::create_participant(MemberId(member_index)),
            );
        }

        state.next_member_id = MemberId(member_limit + 1);

        assert!(GameState::try_from(GameStateSerialOut::from(&state)).is_err());
    }

    #[test]
    fn try_from_rejects_a_negative_world_extent() {
        let mut state_serial_out: GameStateSerialOut =
            GameStateSerialOut::from(&create_state_with_members(GameModeKind::FreeForAll));
        state_serial_out.world.bounds.width = -1;

        assert!(GameState::try_from(state_serial_out).is_err());
    }
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib game_state_serial`

Expected: FAIL; the first error reads `` error[E0425]: cannot find function, tuple struct or tuple variant `Tick` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/protocol/game_state_serial.rs`, replace:

```rust
use bitcode::{Decode, Encode};

use crate::game::{GameModeKind, GameSettings, GameState};
use crate::protocol::MemberSerialOut;
use crate::random::Pcg32;
use crate::round::{RoundPhase, RoundState};
use crate::world::{World, WorldBounds, WorldShapeKind};
```

with:

```rust
use std::collections::BTreeMap;

use bitcode::{Decode, Encode};

use crate::error::AppError;
use crate::game::{GameModeKind, GameSettings, GameState, Tick};
use crate::geometry::Subpixels;
use crate::member;
use crate::member::{Member, MemberId, MemberRoleKind, TeamKind};
use crate::protocol::{MemberSerialOut, RejectionKind};
use crate::protocol::{geometry_serial, protocol_limits};
use crate::random::Pcg32;
use crate::round::{RoundPhase, RoundState};
use crate::world::{World, WorldBounds, WorldShapeKind};
```

In `shared/src/protocol/game_state_serial.rs`, replace:

```rust
            next_member_id: state.next_member_id.0,
        }
    }
}
```

with:

```rust
            next_member_id: state.next_member_id.0,
        }
    }
}

impl TryFrom<GameStateSerialOut> for GameState {
    type Error = AppError;

    fn try_from(state_serial_out: GameStateSerialOut) -> Result<GameState, AppError> {
        let settings: GameSettings = GameSettings::try_from(state_serial_out.settings)?;
        let round: Option<RoundState> = state_serial_out.round.map(RoundState::from);
        let is_survival: bool = settings.mode == GameModeKind::Survival;

        if round.is_some() != is_survival {
            return Err(AppError::new("a game has a round state exactly in survival"));
        }

        let next_member_id: MemberId = MemberId(state_serial_out.next_member_id);
        let members: BTreeMap<MemberId, Member> = convert_members(state_serial_out.members, &settings, next_member_id)?;

        Ok(GameState {
            tick: Tick(state_serial_out.tick),
            settings,
            rng: Pcg32::try_from(state_serial_out.rng)?,
            world: World::try_from(state_serial_out.world)?,
            round,
            members,
            next_member_id,
        })
    }
}
```

In `shared/src/protocol/game_state_serial.rs`, replace:

```rust
            leaderboard_length: settings.leaderboard_length,
        }
    }
}
```

with:

```rust
            leaderboard_length: settings.leaderboard_length,
        }
    }
}

impl TryFrom<GameSettingsSerialOut> for GameSettings {
    type Error = AppError;

    fn try_from(settings_serial_out: GameSettingsSerialOut) -> Result<GameSettings, AppError> {
        let settings: GameSettings = GameSettings {
            title: settings_serial_out.title,
            mode: GameModeKind::from(settings_serial_out.mode),
            world_shape: WorldShapeKind::from(settings_serial_out.world_shape),
            world_width_pixels: settings_serial_out.world_width_pixels,
            world_height_pixels: settings_serial_out.world_height_pixels,
            player_minimum: settings_serial_out.player_minimum,
            player_cap: settings_serial_out.player_cap,
            team_count: settings_serial_out.team_count,
            leaderboard_length: settings_serial_out.leaderboard_length,
        };

        settings.validate().map_err(RejectionKind::to_app_error)?;

        Ok(settings)
    }
}
```

In `shared/src/protocol/game_state_serial.rs`, replace:

```rust
            GameModeKind::Survival => GameModeKindSerial::Survival,
        }
    }
}
```

with:

```rust
            GameModeKind::Survival => GameModeKindSerial::Survival,
        }
    }
}

impl From<GameModeKindSerial> for GameModeKind {
    fn from(mode_serial: GameModeKindSerial) -> GameModeKind {
        match mode_serial {
            GameModeKindSerial::FreeForAll => GameModeKind::FreeForAll,
            GameModeKindSerial::Skirmish => GameModeKind::Skirmish,
            GameModeKindSerial::Survival => GameModeKind::Survival,
        }
    }
}
```

In `shared/src/protocol/game_state_serial.rs`, replace:

```rust
            WorldShapeKind::Ellipse => WorldShapeKindSerial::Ellipse,
        }
    }
}
```

with:

```rust
            WorldShapeKind::Ellipse => WorldShapeKindSerial::Ellipse,
        }
    }
}

impl From<WorldShapeKindSerial> for WorldShapeKind {
    fn from(world_shape_serial: WorldShapeKindSerial) -> WorldShapeKind {
        match world_shape_serial {
            WorldShapeKindSerial::Rectangle => WorldShapeKind::Rectangle,
            WorldShapeKindSerial::Ellipse => WorldShapeKind::Ellipse,
        }
    }
}
```

In `shared/src/protocol/game_state_serial.rs`, replace:

```rust
            increment: rng.increment(),
        }
    }
}
```

with:

```rust
            increment: rng.increment(),
        }
    }
}

impl TryFrom<Pcg32SerialOut> for Pcg32 {
    type Error = AppError;

    fn try_from(rng_serial_out: Pcg32SerialOut) -> Result<Pcg32, AppError> {
        Pcg32::from_parts(rng_serial_out.state, rng_serial_out.increment)
            .ok_or_else(|| AppError::new("the random number generator increment is even"))
    }
}
```

In `shared/src/protocol/game_state_serial.rs`, replace:

```rust
            initial_bounds: WorldBoundsSerialOut::from(&world.initial_bounds),
        }
    }
}
```

with:

```rust
            initial_bounds: WorldBoundsSerialOut::from(&world.initial_bounds),
        }
    }
}

impl TryFrom<WorldSerialOut> for World {
    type Error = AppError;

    fn try_from(world_serial_out: WorldSerialOut) -> Result<World, AppError> {
        Ok(World {
            shape: WorldShapeKind::from(world_serial_out.shape),
            bounds: WorldBounds::try_from(world_serial_out.bounds)?,
            initial_bounds: WorldBounds::try_from(world_serial_out.initial_bounds)?,
        })
    }
}
```

In `shared/src/protocol/game_state_serial.rs`, replace:

```rust
            height: bounds.height.0,
        }
    }
}
```

with:

```rust
            height: bounds.height.0,
        }
    }
}

impl TryFrom<WorldBoundsSerialOut> for WorldBounds {
    type Error = AppError;

    fn try_from(bounds_serial_out: WorldBoundsSerialOut) -> Result<WorldBounds, AppError> {
        Ok(WorldBounds {
            left: Subpixels(geometry_serial::check_coordinate(
                bounds_serial_out.left,
                protocol_limits::COORDINATE_LIMIT_SUBPIXELS,
            )?),
            top: Subpixels(geometry_serial::check_coordinate(
                bounds_serial_out.top,
                protocol_limits::COORDINATE_LIMIT_SUBPIXELS,
            )?),
            width: Subpixels(check_world_extent(bounds_serial_out.width)?),
            height: Subpixels(check_world_extent(bounds_serial_out.height)?),
        })
    }
}
```

In `shared/src/protocol/game_state_serial.rs`, replace:

```rust
            phase_started_at: round.phase_started_at.0,
        }
    }
}
```

with:

```rust
            phase_started_at: round.phase_started_at.0,
        }
    }
}

impl From<RoundStateSerialOut> for RoundState {
    fn from(round_serial_out: RoundStateSerialOut) -> RoundState {
        RoundState {
            phase: RoundPhase::from(round_serial_out.phase),
            phase_started_at: Tick(round_serial_out.phase_started_at),
        }
    }
}
```

In `shared/src/protocol/game_state_serial.rs`, replace:

```rust
            RoundPhase::PostRound => RoundPhaseSerialOut::PostRound,
        }
    }
}
```

with:

```rust
            RoundPhase::PostRound => RoundPhaseSerialOut::PostRound,
        }
    }
}

impl From<RoundPhaseSerialOut> for RoundPhase {
    fn from(phase_serial_out: RoundPhaseSerialOut) -> RoundPhase {
        match phase_serial_out {
            RoundPhaseSerialOut::Waiting => RoundPhase::Waiting,
            RoundPhaseSerialOut::PreRound => RoundPhase::PreRound,
            RoundPhaseSerialOut::Playing => RoundPhase::Playing,
            RoundPhaseSerialOut::PostRound => RoundPhase::PostRound,
        }
    }
}

fn convert_members(
    member_serial_outs: Vec<MemberSerialOut>,
    settings: &GameSettings,
    next_member_id: MemberId,
) -> Result<BTreeMap<MemberId, Member>, AppError> {
    let member_count: u32 = u32::try_from(member_serial_outs.len())?;

    if member_count > protocol_limits::get_member_limit(settings.player_cap) {
        return Err(AppError::new("a game has more members than its member limit"));
    }

    let mut members: BTreeMap<MemberId, Member> = BTreeMap::new();

    for member_serial_out in member_serial_outs {
        let member: Member = Member::try_from(member_serial_out)?;
        let previous_member_id: Option<MemberId> = members.last_key_value().map(|(member_id, _)| *member_id);

        if previous_member_id.is_some_and(|previous_member_id| previous_member_id >= member.member_id) {
            return Err(AppError::new("members are not in strictly ascending member id order"));
        }

        if member.member_id >= next_member_id {
            return Err(AppError::new("a member id is not below the next member id"));
        }

        check_member_team(&member, settings)?;
        members.insert(member.member_id, member);
    }

    Ok(members)
}

/// A Participant has one of the game's teams exactly in skirmish; a Spectator never has a team.
fn check_member_team(member: &Member, settings: &GameSettings) -> Result<(), AppError> {
    let game_teams: Vec<TeamKind> = settings.team_count.map(member::get_game_teams).unwrap_or_default();
    let is_skirmish_participant: bool =
        settings.mode == GameModeKind::Skirmish && member.role == MemberRoleKind::Participant;
    let is_team_valid: bool = match member.team {
        Some(team) => is_skirmish_participant && game_teams.contains(&team),
        None => !is_skirmish_participant,
    };

    if !is_team_valid {
        return Err(AppError::new(
            "a member's team does not match its role and the game's teams",
        ));
    }

    Ok(())
}

fn check_world_extent(extent: i32) -> Result<i32, AppError> {
    if extent < 0 {
        return Err(AppError::new("a world extent is negative"));
    }

    geometry_serial::check_coordinate(extent, protocol_limits::COORDINATE_LIMIT_SUBPIXELS)
}
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib game_state_serial`

Expected: PASS, `test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 276 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/game_state_serial.rs
git status
git commit -m "game state wire conversions"
```

### Task 13: Input bundle wire types

**Files:**
- Create: `shared/src/protocol/input_bundle_serial.rs`
- Modify: `shared/src/protocol/mod.rs`
- Test: inline `mod tests` in `shared/src/protocol/input_bundle_serial.rs`

The broadcast and replay form of `InputBundle` (protocol.md#input-bundle). Receiving checks press bits, cursor coordinates, joiner screen names and at most `MEMBER_LIMIT_HIGHEST` player inputs. An outbound `aim` without a shot press is not rejected: the server copies `aim` from the input that carried the shot press, and the inbound rule of Task 15 guarantees that pairing.

- [ ] **Step 1: Write the failing test**

Create `shared/src/protocol/input_bundle_serial.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::member::{OrganismColorKind, SkinKind};

    fn create_bundle() -> InputBundle {
        let loadout: Loadout = test_fixture::create_loadout();

        InputBundle {
            tick: Tick(12),
            member_events: vec![
                MemberEvent::Joined {
                    member_id: MemberId(3),
                    screen_name: String::from("Blob"),
                    role: MemberRoleKind::Participant,
                    loadout: Some(loadout),
                    team: Some(TeamKind::Green),
                },
                MemberEvent::SpawnRequested {
                    member_id: MemberId(3),
                    loadout,
                    team: Some(TeamKind::Green),
                },
                MemberEvent::Left { member_id: MemberId(1) },
                MemberEvent::AppearanceChanged {
                    member_id: MemberId(2),
                    appearance: Appearance {
                        color: OrganismColorKind::Royal,
                        skin: SkinKind::Grid,
                    },
                },
            ],
            player_inputs: vec![
                PlayerTickInput {
                    member_id: MemberId(0),
                    cursor: WorldPoint { x: 410, y: -3 },
                    ability_presses: AbilityPressSet::FIRST.with(AbilityPressSet::FOURTH),
                    aim: Some(AimVector { x: -20, y: 7 }),
                },
                PlayerTickInput {
                    member_id: MemberId(2),
                    cursor: WorldPoint { x: 5, y: 6 },
                    ability_presses: AbilityPressSet::NONE,
                    aim: None,
                },
            ],
        }
    }

    #[test]
    fn from_copies_the_tick_and_the_press_bits() {
        let bundle_serial_out: InputBundleSerialOut = InputBundleSerialOut::from(&create_bundle());

        assert_eq!(bundle_serial_out.tick, 12);
        assert_eq!(
            bundle_serial_out.player_inputs[0],
            PlayerTickInputSerialOut {
                member_id: 0,
                cursor: WorldPointSerialOut { x: 410, y: -3 },
                ability_presses: 0b1001,
                aim: Some(AimVectorSerial { x: -20, y: 7 }),
            },
        );
    }

    #[test]
    fn try_from_restores_a_bundle_with_every_member_event() {
        let bundle: InputBundle = create_bundle();

        assert_eq!(
            InputBundle::try_from(InputBundleSerialOut::from(&bundle)).unwrap(),
            bundle
        );
    }

    #[test]
    fn try_from_rejects_unknown_press_bits() {
        let mut bundle_serial_out: InputBundleSerialOut = InputBundleSerialOut::from(&create_bundle());
        bundle_serial_out.player_inputs[1].ability_presses = 0b1_0000;

        assert!(InputBundle::try_from(bundle_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_an_invalid_screen_name_of_a_joiner() {
        let mut bundle_serial_out: InputBundleSerialOut = InputBundleSerialOut::from(&create_bundle());
        bundle_serial_out.member_events[0] = MemberEventSerialOut::Joined {
            member_id: 3,
            screen_name: String::new(),
            role: MemberRoleKindSerialOut::Spectator,
            loadout: None,
            team: None,
        };

        assert!(InputBundle::try_from(bundle_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_more_player_inputs_than_any_game_has_members() {
        let mut bundle_serial_out: InputBundleSerialOut = InputBundleSerialOut::from(&create_bundle());
        let player_input_serial_out: PlayerTickInputSerialOut = bundle_serial_out.player_inputs[1];
        bundle_serial_out.player_inputs = vec![player_input_serial_out; 97];

        assert!(InputBundle::try_from(bundle_serial_out).is_err());
    }
}
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub mod geometry_serial;
```

with:

```rust
pub mod geometry_serial;
pub mod input_bundle_serial;
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub use geometry_serial::*;
```

with:

```rust
pub use geometry_serial::*;
pub use input_bundle_serial::*;
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib input_bundle_serial`

Expected: FAIL; the first error reads `` error[E0425]: cannot find type `InputBundle` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/protocol/input_bundle_serial.rs`, insert directly above `#[cfg(test)]`:

```rust
use bitcode::{Decode, Encode};

use crate::ability::{AbilityPressSet, AimVector, Loadout};
use crate::error::AppError;
use crate::game::{InputBundle, MemberEvent, PlayerTickInput, Tick};
use crate::geometry::WorldPoint;
use crate::member::{Appearance, MemberId, MemberRoleKind, TeamKind};
use crate::protocol::protocol_limits;
use crate::protocol::{
    AimVectorSerial, AppearanceSerial, LoadoutSerial, MemberRoleKindSerialOut, RejectionKind, TeamKindSerial,
    WorldPointSerialOut,
};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct InputBundleSerialOut {
    pub tick: u32,
    pub member_events: Vec<MemberEventSerialOut>,
    pub player_inputs: Vec<PlayerTickInputSerialOut>,
}

impl From<&InputBundle> for InputBundleSerialOut {
    fn from(bundle: &InputBundle) -> InputBundleSerialOut {
        InputBundleSerialOut {
            tick: bundle.tick.0,
            member_events: bundle.member_events.iter().map(MemberEventSerialOut::from).collect(),
            player_inputs: bundle.player_inputs.iter().map(PlayerTickInputSerialOut::from).collect(),
        }
    }
}

impl TryFrom<InputBundleSerialOut> for InputBundle {
    type Error = AppError;

    fn try_from(bundle_serial_out: InputBundleSerialOut) -> Result<InputBundle, AppError> {
        let player_input_count: u32 = u32::try_from(bundle_serial_out.player_inputs.len())?;

        if player_input_count > protocol_limits::MEMBER_LIMIT_HIGHEST {
            return Err(AppError::new(
                "a bundle has more player inputs than any game has members",
            ));
        }

        let member_events: Vec<MemberEvent> = bundle_serial_out
            .member_events
            .into_iter()
            .map(MemberEvent::try_from)
            .collect::<Result<Vec<MemberEvent>, AppError>>()?;
        let player_inputs: Vec<PlayerTickInput> = bundle_serial_out
            .player_inputs
            .into_iter()
            .map(PlayerTickInput::try_from)
            .collect::<Result<Vec<PlayerTickInput>, AppError>>()?;

        Ok(InputBundle {
            tick: Tick(bundle_serial_out.tick),
            member_events,
            player_inputs,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PlayerTickInputSerialOut {
    pub member_id: u32,
    pub cursor: WorldPointSerialOut,
    pub ability_presses: u8,
    pub aim: Option<AimVectorSerial>,
}

impl From<&PlayerTickInput> for PlayerTickInputSerialOut {
    fn from(player_input: &PlayerTickInput) -> PlayerTickInputSerialOut {
        PlayerTickInputSerialOut {
            member_id: player_input.member_id.0,
            cursor: WorldPointSerialOut::from(&player_input.cursor),
            ability_presses: player_input.ability_presses.bits(),
            aim: player_input.aim.as_ref().map(AimVectorSerial::from),
        }
    }
}

impl TryFrom<PlayerTickInputSerialOut> for PlayerTickInput {
    type Error = AppError;

    fn try_from(player_input_serial_out: PlayerTickInputSerialOut) -> Result<PlayerTickInput, AppError> {
        let ability_presses: AbilityPressSet = AbilityPressSet::from_bits(player_input_serial_out.ability_presses)
            .ok_or_else(|| AppError::new("unknown ability press bits"))?;

        Ok(PlayerTickInput {
            member_id: MemberId(player_input_serial_out.member_id),
            cursor: WorldPoint::try_from(player_input_serial_out.cursor)?,
            ability_presses,
            aim: player_input_serial_out.aim.map(AimVector::from),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum MemberEventSerialOut {
    Joined {
        member_id: u32,
        screen_name: String,
        role: MemberRoleKindSerialOut,
        loadout: Option<LoadoutSerial>,
        team: Option<TeamKindSerial>,
    },
    Left {
        member_id: u32,
    },
    SpawnRequested {
        member_id: u32,
        loadout: LoadoutSerial,
        team: Option<TeamKindSerial>,
    },
    AppearanceChanged {
        member_id: u32,
        appearance: AppearanceSerial,
    },
}

impl From<&MemberEvent> for MemberEventSerialOut {
    fn from(member_event: &MemberEvent) -> MemberEventSerialOut {
        match member_event {
            MemberEvent::Joined {
                member_id,
                screen_name,
                role,
                loadout,
                team,
            } => MemberEventSerialOut::Joined {
                member_id: member_id.0,
                screen_name: screen_name.clone(),
                role: MemberRoleKindSerialOut::from(role),
                loadout: loadout.as_ref().map(LoadoutSerial::from),
                team: team.as_ref().map(TeamKindSerial::from),
            },
            MemberEvent::Left { member_id } => MemberEventSerialOut::Left { member_id: member_id.0 },
            MemberEvent::SpawnRequested {
                member_id,
                loadout,
                team,
            } => MemberEventSerialOut::SpawnRequested {
                member_id: member_id.0,
                loadout: LoadoutSerial::from(loadout),
                team: team.as_ref().map(TeamKindSerial::from),
            },
            MemberEvent::AppearanceChanged { member_id, appearance } => MemberEventSerialOut::AppearanceChanged {
                member_id: member_id.0,
                appearance: AppearanceSerial::from(appearance),
            },
        }
    }
}

impl TryFrom<MemberEventSerialOut> for MemberEvent {
    type Error = AppError;

    fn try_from(member_event_serial_out: MemberEventSerialOut) -> Result<MemberEvent, AppError> {
        let member_event: MemberEvent = match member_event_serial_out {
            MemberEventSerialOut::Joined {
                member_id,
                screen_name,
                role,
                loadout,
                team,
            } => {
                protocol_limits::check_screen_name(&screen_name).map_err(RejectionKind::to_app_error)?;

                MemberEvent::Joined {
                    member_id: MemberId(member_id),
                    screen_name,
                    role: MemberRoleKind::from(role),
                    loadout: loadout.map(Loadout::from),
                    team: team.map(TeamKind::from),
                }
            }
            MemberEventSerialOut::Left { member_id } => MemberEvent::Left {
                member_id: MemberId(member_id),
            },
            MemberEventSerialOut::SpawnRequested {
                member_id,
                loadout,
                team,
            } => MemberEvent::SpawnRequested {
                member_id: MemberId(member_id),
                loadout: Loadout::from(loadout),
                team: team.map(TeamKind::from),
            },
            MemberEventSerialOut::AppearanceChanged { member_id, appearance } => MemberEvent::AppearanceChanged {
                member_id: MemberId(member_id),
                appearance: Appearance::from(appearance),
            },
        };

        Ok(member_event)
    }
}
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib input_bundle_serial`

Expected: PASS, `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 288 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/input_bundle_serial.rs shared/src/protocol/mod.rs
git status
git commit -m "input bundle wire types"
```

### Task 14: Joiner and game settings request payloads

**Files:**
- Create: `shared/src/member/joiner.rs`
- Create: `shared/src/protocol/message_in.rs`
- Modify: `shared/src/member/mod.rs`
- Modify: `shared/src/protocol/mod.rs`
- Test: inline `mod tests` in `shared/src/protocol/message_in.rs`

The request payloads which convert with `RejectionKind` errors, answered as `RequestRejected`. `GameSettingsSerialIn` carries one world size, copied into both dimensions, and its title is trimmed before `validate`. `TeamChoiceKindSerialIn` is a plain enum mirror, so it converts with `From`; the team itself is checked against the game by the server in phase 5.

- [ ] **Step 1: Write the failing test**

Create `shared/src/protocol/message_in.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::protocol::{RangeBoundKind, SettingFieldKind};

    fn create_settings_serial_in() -> GameSettingsSerialIn {
        GameSettingsSerialIn {
            title: String::from("  Arena  "),
            mode: GameModeKindSerial::Survival,
            world_shape: WorldShapeKindSerial::Ellipse,
            world_size_pixels: 800,
            player_minimum: Some(4),
            player_cap: 16,
            team_count: None,
            leaderboard_length: 10,
        }
    }

    fn create_joiner_serial_in() -> JoinerSerialIn {
        JoinerSerialIn {
            screen_name: String::from(" Blob "),
            loadout: LoadoutSerial::from(&test_fixture::create_loadout()),
            team: TeamChoiceKindSerialIn::Team(TeamKindSerial::Pink),
        }
    }

    #[test]
    fn game_settings_try_from_trims_the_title_and_makes_the_world_square() {
        let settings: GameSettings = GameSettings::try_from(create_settings_serial_in()).unwrap();

        assert_eq!(
            settings,
            GameSettings {
                title: String::from("Arena"),
                mode: GameModeKind::Survival,
                world_shape: WorldShapeKind::Ellipse,
                world_width_pixels: 800,
                world_height_pixels: 800,
                player_minimum: Some(4),
                player_cap: 16,
                team_count: None,
                leaderboard_length: 10,
            },
        );
    }

    #[test]
    fn game_settings_try_from_rejects_a_blank_title() {
        let mut settings_serial_in: GameSettingsSerialIn = create_settings_serial_in();
        settings_serial_in.title = String::from("   ");

        assert_eq!(
            GameSettings::try_from(settings_serial_in),
            Err(RejectionKind::TitleEmpty)
        );
    }

    #[test]
    fn game_settings_try_from_applies_the_settings_rules() {
        let mut small_world_serial_in: GameSettingsSerialIn = create_settings_serial_in();
        small_world_serial_in.world_size_pixels = 299;
        let mut low_cap_serial_in: GameSettingsSerialIn = create_settings_serial_in();
        low_cap_serial_in.player_cap = 3;

        assert_eq!(
            GameSettings::try_from(small_world_serial_in),
            Err(RejectionKind::SettingOutOfRange {
                field: SettingFieldKind::WorldSize,
                bound: RangeBoundKind::Below,
            }),
        );
        assert_eq!(
            GameSettings::try_from(low_cap_serial_in),
            Err(RejectionKind::PlayerCapBelowMinimum)
        );
    }

    #[test]
    fn joiner_try_from_trims_the_screen_name_and_converts_the_team_choice() {
        let joiner: Joiner = Joiner::try_from(create_joiner_serial_in()).unwrap();

        assert_eq!(
            joiner,
            Joiner {
                screen_name: String::from("Blob"),
                loadout: test_fixture::create_loadout(),
                team: TeamChoiceKind::Team(TeamKind::Pink),
            },
        );
    }

    #[test]
    fn joiner_try_from_rejects_an_invalid_screen_name() {
        let mut joiner_serial_in: JoinerSerialIn = create_joiner_serial_in();
        joiner_serial_in.screen_name = "n".repeat(65);

        assert_eq!(
            Joiner::try_from(joiner_serial_in),
            Err(RejectionKind::ScreenNameTooLong)
        );
    }

    #[test]
    fn team_choice_kind_from_converts_auto() {
        assert_eq!(TeamChoiceKind::from(TeamChoiceKindSerialIn::Auto), TeamChoiceKind::Auto);
    }
}
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub mod member_serial;
```

with:

```rust
pub mod member_serial;
pub mod message_in;
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub use member_serial::*;
```

with:

```rust
pub use member_serial::*;
pub use message_in::*;
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib message_in`

Expected: FAIL; the first error reads `` error[E0425]: cannot find type `GameSettingsSerialIn` in this scope ``.

- [ ] **Step 3: Implement**

Create `shared/src/member/joiner.rs`:

```rust
use crate::ability::Loadout;
use crate::member::TeamKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Joiner {
    /// Trimmed.
    pub screen_name: String,
    pub loadout: Loadout,
    pub team: TeamChoiceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamChoiceKind {
    Auto,
    Team(TeamKind),
}
```

In `shared/src/protocol/message_in.rs`, insert directly above `#[cfg(test)]`:

```rust
use bitcode::{Decode, Encode};

use crate::ability::Loadout;
use crate::game::{GameModeKind, GameSettings};
use crate::member::{Joiner, TeamChoiceKind, TeamKind};
use crate::protocol::protocol_limits;
use crate::protocol::{GameModeKindSerial, LoadoutSerial, RejectionKind, TeamKindSerial, WorldShapeKindSerial};
use crate::world::WorldShapeKind;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct GameSettingsSerialIn {
    pub title: String,
    pub mode: GameModeKindSerial,
    pub world_shape: WorldShapeKindSerial,
    pub world_size_pixels: u32,
    pub player_minimum: Option<u8>,
    pub player_cap: u8,
    pub team_count: Option<u8>,
    pub leaderboard_length: u8,
}

impl TryFrom<GameSettingsSerialIn> for GameSettings {
    type Error = RejectionKind;

    fn try_from(settings_serial_in: GameSettingsSerialIn) -> Result<GameSettings, RejectionKind> {
        let settings: GameSettings = GameSettings {
            title: protocol_limits::normalize_title(&settings_serial_in.title)?,
            mode: GameModeKind::from(settings_serial_in.mode),
            world_shape: WorldShapeKind::from(settings_serial_in.world_shape),
            // Created games are square.
            world_width_pixels: settings_serial_in.world_size_pixels,
            world_height_pixels: settings_serial_in.world_size_pixels,
            player_minimum: settings_serial_in.player_minimum,
            player_cap: settings_serial_in.player_cap,
            team_count: settings_serial_in.team_count,
            leaderboard_length: settings_serial_in.leaderboard_length,
        };

        settings.validate()?;

        Ok(settings)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct JoinerSerialIn {
    pub screen_name: String,
    pub loadout: LoadoutSerial,
    pub team: TeamChoiceKindSerialIn,
}

impl TryFrom<JoinerSerialIn> for Joiner {
    type Error = RejectionKind;

    fn try_from(joiner_serial_in: JoinerSerialIn) -> Result<Joiner, RejectionKind> {
        Ok(Joiner {
            screen_name: protocol_limits::normalize_screen_name(&joiner_serial_in.screen_name)?,
            loadout: Loadout::from(joiner_serial_in.loadout),
            team: TeamChoiceKind::from(joiner_serial_in.team),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub enum TeamChoiceKindSerialIn {
    Auto,
    Team(TeamKindSerial),
}

impl From<TeamChoiceKindSerialIn> for TeamChoiceKind {
    fn from(team_choice_serial_in: TeamChoiceKindSerialIn) -> TeamChoiceKind {
        match team_choice_serial_in {
            TeamChoiceKindSerialIn::Auto => TeamChoiceKind::Auto,
            TeamChoiceKindSerialIn::Team(team_serial) => TeamChoiceKind::Team(TeamKind::from(team_serial)),
        }
    }
}
```

In `shared/src/member/mod.rs`, replace:

```rust
pub mod member;
```

with:

```rust
pub mod joiner;
pub mod member;
```

In `shared/src/member/mod.rs`, replace:

```rust
pub use member::*;
```

with:

```rust
pub use joiner::*;
pub use member::*;
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib message_in`

Expected: PASS, `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 293 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/member/joiner.rs shared/src/member/mod.rs shared/src/protocol/message_in.rs shared/src/protocol/mod.rs
git status
git commit -m "joiner and game settings request payloads"
```

### Task 15: Player input and the inbound message

**Files:**
- Create: `shared/src/game/player_input.rs`
- Modify: `shared/src/game/mod.rs`
- Modify: `shared/src/protocol/message_in.rs`
- Modify: `shared/src/protocol/input_bundle_serial.rs`
- Test: inline `mod tests` in `shared/src/protocol/message_in.rs`

`PlayerInputSerialIn` converts with `AppError`, since a malformed input closes the connection (server.md#input-validation-and-speed-clamp): known press bits only, an aim only with a first or second press (either may be a shot slot; the loadout is the server's to know), a cursor within the coordinate limit. The press bit check is shared with the bundle conversion. `MessageSerialIn` is wire-only.

- [ ] **Step 1: Write the failing test**

In `shared/src/protocol/message_in.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    fn create_player_input_serial_in(ability_presses: u8, aim: Option<AimVectorSerial>) -> PlayerInputSerialIn {
        PlayerInputSerialIn {
            client_tick: 41,
            cursor: SubpixelPointSerial { x: 409_600, y: -1 },
            ability_presses,
            aim,
        }
    }

    #[test]
    fn player_input_try_from_accepts_an_aim_with_a_shot_slot_press() {
        let aim_serial: AimVectorSerial = AimVectorSerial { x: 30, y: -4 };
        let player_input: PlayerInput =
            PlayerInput::try_from(create_player_input_serial_in(0b0010, Some(aim_serial))).unwrap();

        assert_eq!(
            player_input,
            PlayerInput {
                client_tick: Tick(41),
                cursor: SubpixelPoint { x: 409_600, y: -1 },
                ability_presses: AbilityPressSet::SECOND,
                aim: Some(AimVector { x: 30, y: -4 }),
            },
        );
        assert!(PlayerInput::try_from(create_player_input_serial_in(0b0001, Some(aim_serial))).is_ok());
        assert!(PlayerInput::try_from(create_player_input_serial_in(0b0001, None)).is_ok());
    }

    #[test]
    fn player_input_try_from_rejects_an_aim_without_a_shot_slot_press() {
        let aim_serial: AimVectorSerial = AimVectorSerial { x: 30, y: -4 };

        assert!(PlayerInput::try_from(create_player_input_serial_in(0b1100, Some(aim_serial))).is_err());
    }

    #[test]
    fn player_input_try_from_rejects_unknown_press_bits() {
        assert!(PlayerInput::try_from(create_player_input_serial_in(0b1000_0000, None)).is_err());
    }

    #[test]
    fn player_input_try_from_rejects_a_cursor_beyond_the_coordinate_limit() {
        let mut player_input_serial_in: PlayerInputSerialIn = create_player_input_serial_in(0, None);
        player_input_serial_in.cursor.y = 268_435_457;

        assert!(PlayerInput::try_from(player_input_serial_in).is_err());
    }
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib message_in`

Expected: FAIL; the first error reads `` error[E0425]: cannot find type `AimVectorSerial` in this scope ``.

- [ ] **Step 3: Implement**

Create `shared/src/game/player_input.rs`:

```rust
use crate::ability::{AbilityPressSet, AimVector};
use crate::game::Tick;
use crate::geometry::SubpixelPoint;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerInput {
    /// Tick of the last bundle the client applied when it sampled.
    pub client_tick: Tick,
    pub cursor: SubpixelPoint,
    pub ability_presses: AbilityPressSet,
    /// Present only with a first or second press.
    pub aim: Option<AimVector>,
}
```

In `shared/src/protocol/input_bundle_serial.rs`, replace:

```rust
        let ability_presses: AbilityPressSet = AbilityPressSet::from_bits(player_input_serial_out.ability_presses)
            .ok_or_else(|| AppError::new("unknown ability press bits"))?;

        Ok(PlayerTickInput {
            member_id: MemberId(player_input_serial_out.member_id),
            cursor: WorldPoint::try_from(player_input_serial_out.cursor)?,
            ability_presses,
```

with:

```rust
        Ok(PlayerTickInput {
            member_id: MemberId(player_input_serial_out.member_id),
            cursor: WorldPoint::try_from(player_input_serial_out.cursor)?,
            ability_presses: convert_ability_presses(player_input_serial_out.ability_presses)?,
```

In `shared/src/protocol/input_bundle_serial.rs`, insert directly above `#[cfg(test)]`:

```rust
pub fn convert_ability_presses(ability_press_bits: u8) -> Result<AbilityPressSet, AppError> {
    AbilityPressSet::from_bits(ability_press_bits).ok_or_else(|| AppError::new("unknown ability press bits"))
}
```

In `shared/src/protocol/message_in.rs`, replace:

```rust
use bitcode::{Decode, Encode};

use crate::ability::Loadout;
use crate::game::{GameModeKind, GameSettings};
use crate::member::{Joiner, TeamChoiceKind, TeamKind};
use crate::protocol::protocol_limits;
use crate::protocol::{GameModeKindSerial, LoadoutSerial, RejectionKind, TeamKindSerial, WorldShapeKindSerial};
use crate::world::WorldShapeKind;
```

with:

```rust
use bitcode::{Decode, Encode};

use crate::ability::{AbilityPressSet, AimVector, Loadout};
use crate::error::AppError;
use crate::game::{GameModeKind, GameSettings, PlayerInput, Tick};
use crate::geometry::SubpixelPoint;
use crate::member::{Joiner, TeamChoiceKind, TeamKind};
use crate::protocol::{
    AimVectorSerial, AppearanceSerial, GameModeKindSerial, LoadoutSerial, RejectionKind, SubpixelPointSerial,
    TeamKindSerial, WorldShapeKindSerial,
};
use crate::protocol::{input_bundle_serial, protocol_limits};
use crate::world::WorldShapeKind;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum MessageSerialIn {
    SubscribeGameList,
    UnsubscribeGameList,
    CreateGame {
        settings: GameSettingsSerialIn,
        password: Option<String>,
        joiner: JoinerSerialIn,
    },
    JoinGame {
        game_id: u32,
        password: Option<String>,
        joiner: JoinerSerialIn,
    },
    SpectateGame {
        game_id: u32,
        password: Option<String>,
        screen_name: String,
    },
    Respawn {
        loadout: LoadoutSerial,
        team: TeamChoiceKindSerialIn,
    },
    UpdateAppearance {
        appearance: AppearanceSerial,
    },
    Input(PlayerInputSerialIn),
    RequestSnapshot {
        client_tick: u32,
    },
    LeaveGame,
}
```

In `shared/src/protocol/message_in.rs`, insert directly above `#[cfg(test)]`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct PlayerInputSerialIn {
    pub client_tick: u32,
    pub cursor: SubpixelPointSerial,
    pub ability_presses: u8,
    pub aim: Option<AimVectorSerial>,
}

impl TryFrom<PlayerInputSerialIn> for PlayerInput {
    type Error = AppError;

    fn try_from(player_input_serial_in: PlayerInputSerialIn) -> Result<PlayerInput, AppError> {
        let ability_presses: AbilityPressSet =
            input_bundle_serial::convert_ability_presses(player_input_serial_in.ability_presses)?;
        let has_shot_slot_press: bool =
            ability_presses.contains(AbilityPressSet::FIRST) || ability_presses.contains(AbilityPressSet::SECOND);

        if player_input_serial_in.aim.is_some() && !has_shot_slot_press {
            return Err(AppError::new("an aim comes only with a first or second ability press"));
        }

        Ok(PlayerInput {
            client_tick: Tick(player_input_serial_in.client_tick),
            cursor: SubpixelPoint::try_from(player_input_serial_in.cursor)?,
            ability_presses,
            aim: player_input_serial_in.aim.map(AimVector::from),
        })
    }
}
```

In `shared/src/game/mod.rs`, replace:

```rust
pub mod input_bundle;
```

with:

```rust
pub mod input_bundle;
pub mod player_input;
```

In `shared/src/game/mod.rs`, replace:

```rust
pub use input_bundle::*;
```

with:

```rust
pub use input_bundle::*;
pub use player_input::*;
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib protocol::`

Expected: PASS, `test result: ok. 68 passed; 0 failed; 0 ignored; 0 measured; 235 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/game/player_input.rs shared/src/game/mod.rs shared/src/protocol/message_in.rs shared/src/protocol/input_bundle_serial.rs
git status
git commit -m "player input and inbound messages"
```

### Task 16: Game summaries

**Files:**
- Create: `shared/src/game/game_summary.rs`
- Create: `shared/src/protocol/lobby.rs`
- Modify: `shared/src/game/mod.rs`
- Modify: `shared/src/protocol/mod.rs`
- Test: inline `mod tests` in `shared/src/protocol/lobby.rs`

`GameId` and the lobby model `GameSummary` (protocol.md#server-to-client) with their wire type. `GameId` derives `Hash` for the `GAMES` registry key of phase 5. Building a summary from a running game is the game task's (phase 5).

- [ ] **Step 1: Write the failing test**

Create `shared/src/protocol/lobby.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn create_game_summary() -> GameSummary {
        GameSummary {
            game_id: GameId(7),
            title: String::from("Arena"),
            mode: GameModeKind::Skirmish,
            player_count: 3,
            spectator_count: 2,
            player_cap: 8,
            secured: true,
            team_sizes: vec![2, 1],
        }
    }

    #[test]
    fn try_from_restores_the_game_summary() {
        let game_summary: GameSummary = create_game_summary();

        assert_eq!(
            GameSummary::try_from(GameSummarySerialOut::from(&game_summary)).unwrap(),
            game_summary
        );
    }

    #[test]
    fn try_from_rejects_an_invalid_title() {
        let mut game_summary_serial_out: GameSummarySerialOut = GameSummarySerialOut::from(&create_game_summary());
        game_summary_serial_out.title = String::new();

        assert!(GameSummary::try_from(game_summary_serial_out).is_err());
    }

    #[test]
    fn try_from_rejects_more_teams_than_a_game_can_have() {
        let mut game_summary_serial_out: GameSummarySerialOut = GameSummarySerialOut::from(&create_game_summary());
        game_summary_serial_out.team_sizes = vec![1; 5];

        assert!(GameSummary::try_from(game_summary_serial_out).is_err());
    }
}
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub mod input_bundle_serial;
```

with:

```rust
pub mod input_bundle_serial;
pub mod lobby;
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub use input_bundle_serial::*;
```

with:

```rust
pub use input_bundle_serial::*;
pub use lobby::*;
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib lobby`

Expected: FAIL; the first error reads `` error[E0425]: cannot find type `GameSummary` in this scope ``.

- [ ] **Step 3: Implement**

Create `shared/src/game/game_summary.rs`:

```rust
use crate::game::GameModeKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GameId(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSummary {
    pub game_id: GameId,
    pub title: String,
    pub mode: GameModeKind,
    /// Alive organisms.
    pub player_count: u8,
    /// Members without an organism.
    pub spectator_count: u8,
    pub player_cap: u8,
    pub secured: bool,
    /// Participants per team, in team order.
    pub team_sizes: Vec<u8>,
}
```

In `shared/src/protocol/lobby.rs`, insert directly above `#[cfg(test)]`:

```rust
use bitcode::{Decode, Encode};

use crate::error::AppError;
use crate::game::{GameId, GameModeKind, GameSummary};
use crate::protocol::protocol_limits;
use crate::protocol::{GameModeKindSerial, RejectionKind};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct GameSummarySerialOut {
    pub game_id: u32,
    pub title: String,
    pub mode: GameModeKindSerial,
    pub player_count: u8,
    pub spectator_count: u8,
    pub player_cap: u8,
    pub secured: bool,
    pub team_sizes: Vec<u8>,
}

impl From<&GameSummary> for GameSummarySerialOut {
    fn from(game_summary: &GameSummary) -> GameSummarySerialOut {
        GameSummarySerialOut {
            game_id: game_summary.game_id.0,
            title: game_summary.title.clone(),
            mode: GameModeKindSerial::from(&game_summary.mode),
            player_count: game_summary.player_count,
            spectator_count: game_summary.spectator_count,
            player_cap: game_summary.player_cap,
            secured: game_summary.secured,
            team_sizes: game_summary.team_sizes.clone(),
        }
    }
}

impl TryFrom<GameSummarySerialOut> for GameSummary {
    type Error = AppError;

    fn try_from(game_summary_serial_out: GameSummarySerialOut) -> Result<GameSummary, AppError> {
        protocol_limits::check_title(&game_summary_serial_out.title).map_err(RejectionKind::to_app_error)?;

        if game_summary_serial_out.team_sizes.len() > usize::from(protocol_limits::TEAM_COUNT_HIGHEST) {
            return Err(AppError::new("a game summary lists more teams than a game can have"));
        }

        Ok(GameSummary {
            game_id: GameId(game_summary_serial_out.game_id),
            title: game_summary_serial_out.title,
            mode: GameModeKind::from(game_summary_serial_out.mode),
            player_count: game_summary_serial_out.player_count,
            spectator_count: game_summary_serial_out.spectator_count,
            player_cap: game_summary_serial_out.player_cap,
            secured: game_summary_serial_out.secured,
            team_sizes: game_summary_serial_out.team_sizes,
        })
    }
}
```

In `shared/src/game/mod.rs`, replace:

```rust
pub mod game_state;
```

with:

```rust
pub mod game_state;
pub mod game_summary;
```

In `shared/src/game/mod.rs`, replace:

```rust
pub use game_state::*;
```

with:

```rust
pub use game_state::*;
pub use game_summary::*;
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib lobby`

Expected: PASS, `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 303 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/game/game_summary.rs shared/src/protocol/lobby.rs shared/src/game/mod.rs shared/src/protocol/mod.rs
git status
git commit -m "game summary wire type"
```

### Task 17: Outbound message and the message codec

**Files:**
- Create: `shared/src/protocol/message_out.rs`
- Modify: `shared/src/protocol/protocol.rs`
- Modify: `shared/src/protocol/mod.rs`
- Test: inline `mod tests` in `shared/src/protocol/protocol.rs`

`MessageSerialOut` and `GameSnapshotSerialOut` are wire-only. `protocol.rs` holds the only `bitcode::encode` and `bitcode::decode` calls of the workspace; a decode error becomes an `AppError` carrying bitcode's error as its source. The round-trip tests cover every message variant and, through them, every wire type except the replay header (Task 18).

- [ ] **Step 1: Write the failing test**

Append to the end of `shared/src/protocol/protocol.rs`, after one blank line:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, GameState, Tick, test_fixture};
    use crate::geometry::{LatticeCoordinate, WorldPoint};
    use crate::member::{Member, MemberId, TeamKind};
    use crate::organism::CellOccupancy;
    use crate::protocol::{
        AimVectorSerial, AppearanceSerial, CellOccupancySerialOut, GameModeKindSerial, GameSettingsSerialIn,
        GameSnapshotSerialOut, GameSummarySerialOut, InputBundleSerialOut, JoinerSerialIn, LoadoutSerial,
        MemberEventSerialOut, MemberRoleKindSerialOut, OrganismColorKindSerial, PlayerInputSerialIn,
        PlayerTickInputSerialOut, RangeBoundKindSerialOut, RejectionKindSerialOut, RequestKindSerialOut,
        SettingFieldKindSerialOut, SkinKindSerial, SubpixelPointSerial, TeamChoiceKindSerialIn, TeamKindSerial,
        WorldPointSerialOut, WorldShapeKindSerial,
    };
    use crate::world::WorldShapeKind;

    fn create_state_serial_out() -> GameStateSerialOut {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, WorldShapeKind::Ellipse, 1200);
        let mut member: Member = test_fixture::create_participant(MemberId(2));
        member.organism = Some(test_fixture::create_organism_with_projectiles());
        state.members.insert(MemberId(2), member);
        state.members.insert(MemberId(4), test_fixture::create_participant(MemberId(4)));
        state.next_member_id = MemberId(5);
        state.tick = Tick(77);

        GameStateSerialOut::from(&state)
    }

    fn create_loadout_serial() -> LoadoutSerial {
        LoadoutSerial::from(&test_fixture::create_loadout())
    }

    fn create_joiner_serial_in() -> JoinerSerialIn {
        JoinerSerialIn {
            screen_name: String::from("Blob"),
            loadout: create_loadout_serial(),
            team: TeamChoiceKindSerialIn::Team(TeamKindSerial::Blue),
        }
    }

    fn create_bundle_serial_out() -> InputBundleSerialOut {
        InputBundleSerialOut {
            tick: 78,
            member_events: vec![
                MemberEventSerialOut::Joined {
                    member_id: 5,
                    screen_name: String::from("Late"),
                    role: MemberRoleKindSerialOut::Participant,
                    loadout: Some(create_loadout_serial()),
                    team: None,
                },
                MemberEventSerialOut::SpawnRequested {
                    member_id: 5,
                    loadout: create_loadout_serial(),
                    team: Some(TeamKindSerial::Red),
                },
                MemberEventSerialOut::Left { member_id: 4 },
                MemberEventSerialOut::AppearanceChanged {
                    member_id: 2,
                    appearance: AppearanceSerial {
                        color: OrganismColorKindSerial::Sun,
                        skin: SkinKindSerial::None,
                    },
                },
            ],
            player_inputs: vec![PlayerTickInputSerialOut {
                member_id: 2,
                cursor: WorldPointSerialOut { x: 41, y: 52 },
                ability_presses: 0b0011,
                aim: Some(AimVectorSerial { x: 9, y: -9 }),
            }],
        }
    }

    #[test]
    fn decode_message_in_restores_every_variant() {
        let message_serial_ins: Vec<MessageSerialIn> = vec![
            MessageSerialIn::SubscribeGameList,
            MessageSerialIn::UnsubscribeGameList,
            MessageSerialIn::CreateGame {
                settings: GameSettingsSerialIn {
                    title: String::from("Arena"),
                    mode: GameModeKindSerial::Skirmish,
                    world_shape: WorldShapeKindSerial::Rectangle,
                    world_size_pixels: 800,
                    player_minimum: None,
                    player_cap: 16,
                    team_count: Some(2),
                    leaderboard_length: 10,
                },
                password: Some(String::from("secret")),
                joiner: create_joiner_serial_in(),
            },
            MessageSerialIn::JoinGame {
                game_id: 3,
                password: None,
                joiner: create_joiner_serial_in(),
            },
            MessageSerialIn::SpectateGame {
                game_id: 3,
                password: Some(String::from("secret")),
                screen_name: String::from("Watcher"),
            },
            MessageSerialIn::Respawn {
                loadout: create_loadout_serial(),
                team: TeamChoiceKindSerialIn::Auto,
            },
            MessageSerialIn::UpdateAppearance {
                appearance: create_loadout_serial().appearance,
            },
            MessageSerialIn::Input(PlayerInputSerialIn {
                client_tick: 90,
                cursor: SubpixelPointSerial { x: -2048, y: 999 },
                ability_presses: 0b0001,
                aim: Some(AimVectorSerial { x: 1, y: 2 }),
            }),
            MessageSerialIn::RequestSnapshot { client_tick: 91 },
            MessageSerialIn::LeaveGame,
        ];

        for message_serial_in in message_serial_ins {
            let message_bytes: Vec<u8> = encode_message_in(&message_serial_in);

            assert_eq!(decode_message_in(&message_bytes).unwrap(), message_serial_in);
        }
    }

    #[test]
    fn decode_message_out_restores_every_variant() {
        let message_serial_outs: Vec<MessageSerialOut> = vec![
            MessageSerialOut::GameList {
                game_summaries: vec![GameSummarySerialOut {
                    game_id: 3,
                    title: String::from("Arena"),
                    mode: GameModeKindSerial::Survival,
                    player_count: 4,
                    spectator_count: 1,
                    player_cap: 16,
                    secured: false,
                    team_sizes: Vec::new(),
                }],
                online_client_count: 12,
            },
            MessageSerialOut::JoinAccepted {
                game_id: 3,
                member_id: 5,
            },
            MessageSerialOut::Snapshot(GameSnapshotSerialOut {
                game_id: 3,
                state: create_state_serial_out(),
            }),
            MessageSerialOut::InputBundle(create_bundle_serial_out()),
            MessageSerialOut::StateChecksum {
                tick: 84,
                checksum: 0xfedc_ba98_7654_3210,
            },
            MessageSerialOut::CursorCorrected {
                client_tick: 80,
                cursor: SubpixelPointSerial { x: 3, y: -3 },
            },
            MessageSerialOut::RequestRejected {
                request: RequestKindSerialOut::JoinGame,
                reason: RejectionKindSerialOut::TeamUnbalanced {
                    requested: TeamKindSerial::Green,
                    smaller: TeamKindSerial::Pink,
                },
            },
            MessageSerialOut::RequestRejected {
                request: RequestKindSerialOut::CreateGame,
                reason: RejectionKindSerialOut::SettingOutOfRange {
                    field: SettingFieldKindSerialOut::LeaderboardLength,
                    bound: RangeBoundKindSerialOut::Above,
                },
            },
            MessageSerialOut::LeftGame,
            MessageSerialOut::GameEnded,
        ];

        for message_serial_out in message_serial_outs {
            let message_bytes: Vec<u8> = encode_message_out(&message_serial_out);

            assert_eq!(decode_message_out(&message_bytes).unwrap(), message_serial_out);
        }
    }

    #[test]
    fn decode_game_state_restores_the_state() {
        let state_serial_out: GameStateSerialOut = create_state_serial_out();
        let state_bytes: Vec<u8> = encode_game_state(&state_serial_out);

        assert_eq!(decode_game_state(&state_bytes).unwrap(), state_serial_out);
    }

    #[test]
    fn decode_message_out_rejects_a_truncated_or_extended_frame() {
        let message_bytes: Vec<u8> = encode_message_out(&MessageSerialOut::InputBundle(create_bundle_serial_out()));
        let mut extended_bytes: Vec<u8> = message_bytes.clone();
        extended_bytes.push(0);

        assert!(decode_message_out(&message_bytes[..message_bytes.len() - 1]).is_err());
        assert!(decode_message_out(&extended_bytes).is_err());
    }

    #[test]
    fn encode_gives_equal_bytes_for_equal_cell_sets_built_in_different_orders() {
        let mut first_cells: CellOccupancy = CellOccupancy::empty();
        let mut second_cells: CellOccupancy = CellOccupancy::empty();

        for (i, j) in [(0, 0), (3, -2), (-1, 4), (2, 2)] {
            first_cells.insert(LatticeCoordinate { i, j });
        }

        for (i, j) in [(2, 2), (-6, 0), (-1, 4), (0, 0), (3, -2)] {
            second_cells.insert(LatticeCoordinate { i, j });
        }

        second_cells.remove(LatticeCoordinate { i: -6, j: 0 });
        second_cells.retighten();

        let first_bytes: Vec<u8> = bitcode::encode(&CellOccupancySerialOut::from(&first_cells));
        let second_bytes: Vec<u8> = bitcode::encode(&CellOccupancySerialOut::from(&second_cells));

        assert_eq!(first_bytes, second_bytes);
    }

    #[test]
    fn decode_game_state_then_try_from_restores_the_model() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);
        let mut member: Member = test_fixture::create_participant_with_organism(MemberId(0), WorldPoint { x: 9, y: 9 });
        member.team = Some(TeamKind::Red);
        state.members.insert(MemberId(0), member);
        state.next_member_id = MemberId(1);

        let state_bytes: Vec<u8> = encode_game_state(&GameStateSerialOut::from(&state));
        let decoded_state: GameState = GameState::try_from(decode_game_state(&state_bytes).unwrap()).unwrap();

        assert_eq!(decoded_state, state);
    }
}
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib protocol::protocol::`

Expected: FAIL; the first error reads `` error[E0432]: unresolved import `crate::protocol::GameSnapshotSerialOut` ``.

- [ ] **Step 3: Implement**

Create `shared/src/protocol/message_out.rs`:

```rust
use bitcode::{Decode, Encode};

use crate::protocol::{
    GameStateSerialOut, GameSummarySerialOut, InputBundleSerialOut, RejectionKindSerialOut, RequestKindSerialOut,
    SubpixelPointSerial,
};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum MessageSerialOut {
    GameList {
        game_summaries: Vec<GameSummarySerialOut>,
        online_client_count: u32,
    },
    JoinAccepted {
        game_id: u32,
        member_id: u32,
    },
    Snapshot(GameSnapshotSerialOut),
    InputBundle(InputBundleSerialOut),
    StateChecksum {
        tick: u32,
        checksum: u64,
    },
    CursorCorrected {
        client_tick: u32,
        cursor: SubpixelPointSerial,
    },
    RequestRejected {
        request: RequestKindSerialOut,
        reason: RejectionKindSerialOut,
    },
    LeftGame,
    GameEnded,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct GameSnapshotSerialOut {
    pub game_id: u32,
    pub state: GameStateSerialOut,
}
```

In `shared/src/protocol/protocol.rs`, replace everything above `#[cfg(test)]` with:

```rust
use bitcode::DecodeOwned;

use crate::error::AppError;
use crate::protocol::{GameStateSerialOut, MessageSerialIn, MessageSerialOut};

/// Bumped on any change to a wire type.
pub const PROTOCOL_VERSION: u16 = 1;

pub fn encode_message_in(message_serial_in: &MessageSerialIn) -> Vec<u8> {
    bitcode::encode(message_serial_in)
}

pub fn decode_message_in(bytes: &[u8]) -> Result<MessageSerialIn, AppError> {
    decode(bytes, "client message")
}

pub fn encode_message_out(message_serial_out: &MessageSerialOut) -> Vec<u8> {
    bitcode::encode(message_serial_out)
}

pub fn decode_message_out(bytes: &[u8]) -> Result<MessageSerialOut, AppError> {
    decode(bytes, "server message")
}

pub fn encode_game_state(state_serial_out: &GameStateSerialOut) -> Vec<u8> {
    bitcode::encode(state_serial_out)
}

pub fn decode_game_state(bytes: &[u8]) -> Result<GameStateSerialOut, AppError> {
    decode(bytes, "game state")
}

fn decode<T: DecodeOwned>(bytes: &[u8], subject: &str) -> Result<T, AppError> {
    bitcode::decode(bytes)
        .map_err(|error| AppError::from_error(&format!("cannot decode the {subject}"), Box::new(error)))
}
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub mod message_in;
```

with:

```rust
pub mod message_in;
pub mod message_out;
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub use message_in::*;
```

with:

```rust
pub use message_in::*;
pub use message_out::*;
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib protocol::protocol::`

Expected: PASS, `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 306 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/protocol/message_out.rs shared/src/protocol/protocol.rs shared/src/protocol/mod.rs
git status
git commit -m "message codec"
```

### Task 18: Replay header

**Files:**
- Create: `shared/src/replay/replay.rs`
- Create: `shared/src/replay/mod.rs`
- Create: `shared/src/protocol/replay_serial.rs`
- Modify: `shared/src/lib.rs`
- Modify: `shared/src/protocol/protocol.rs`
- Modify: `shared/src/protocol/mod.rs`
- Test: inline `mod tests` in `shared/src/protocol/replay_serial.rs` and `shared/src/protocol/protocol.rs`

Replay files are server output, so their header is `ReplayHeaderSerialOut` (protocol.md#wire-types); its model `ReplayHeader` opens the `replay` module, which phase 5's capture and the replay tests share. The replay header and bundle records have their own encode and decode functions, since a record is not a message frame.

- [ ] **Step 1: Write the failing test**

Create `shared/src/protocol/replay_serial.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, test_fixture};
    use crate::world::WorldShapeKind;

    fn create_header() -> ReplayHeader {
        ReplayHeader {
            settings: test_fixture::create_settings(GameModeKind::Skirmish, WorldShapeKind::Ellipse, 900),
            seed: 0x0123_4567_89ab_cdef,
        }
    }

    #[test]
    fn try_from_restores_the_header() {
        let header: ReplayHeader = create_header();

        assert_eq!(
            ReplayHeader::try_from(ReplayHeaderSerialOut::from(&header)).unwrap(),
            header
        );
    }

    #[test]
    fn try_from_rejects_invalid_settings() {
        let mut header_serial_out: ReplayHeaderSerialOut = ReplayHeaderSerialOut::from(&create_header());
        header_serial_out.settings.team_count = None;

        assert!(ReplayHeader::try_from(header_serial_out).is_err());
    }
}
```

In `shared/src/protocol/protocol.rs`, replace:

```rust
    use crate::protocol::{
        AimVectorSerial, AppearanceSerial, CellOccupancySerialOut, GameModeKindSerial, GameSettingsSerialIn,
        GameSnapshotSerialOut, GameSummarySerialOut, InputBundleSerialOut, JoinerSerialIn, LoadoutSerial,
        MemberEventSerialOut, MemberRoleKindSerialOut, OrganismColorKindSerial, PlayerInputSerialIn,
        PlayerTickInputSerialOut, RangeBoundKindSerialOut, RejectionKindSerialOut, RequestKindSerialOut,
        SettingFieldKindSerialOut, SkinKindSerial, SubpixelPointSerial, TeamChoiceKindSerialIn, TeamKindSerial,
        WorldPointSerialOut, WorldShapeKindSerial,
    };
```

with:

```rust
    use crate::protocol::{
        AimVectorSerial, AppearanceSerial, CellOccupancySerialOut, GameModeKindSerial, GameSettingsSerialIn,
        GameSettingsSerialOut, GameSnapshotSerialOut, GameSummarySerialOut, InputBundleSerialOut, JoinerSerialIn,
        LoadoutSerial, MemberEventSerialOut, MemberRoleKindSerialOut, OrganismColorKindSerial, PlayerInputSerialIn,
        PlayerTickInputSerialOut, RangeBoundKindSerialOut, RejectionKindSerialOut, RequestKindSerialOut,
        SettingFieldKindSerialOut, SkinKindSerial, SubpixelPointSerial, TeamChoiceKindSerialIn, TeamKindSerial,
        WorldPointSerialOut, WorldShapeKindSerial,
    };
```

In `shared/src/protocol/protocol.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn decode_replay_header_restores_the_header() {
        let header_serial_out: ReplayHeaderSerialOut = ReplayHeaderSerialOut {
            settings: GameSettingsSerialOut::from(&test_fixture::create_settings(
                GameModeKind::FreeForAll,
                WorldShapeKind::Rectangle,
                640,
            )),
            seed: 99,
        };
        let header_bytes: Vec<u8> = encode_replay_header(&header_serial_out);

        assert_eq!(decode_replay_header(&header_bytes).unwrap(), header_serial_out);
    }

    #[test]
    fn decode_replay_bundle_restores_the_bundle() {
        let bundle_serial_out: InputBundleSerialOut = create_bundle_serial_out();
        let bundle_bytes: Vec<u8> = encode_replay_bundle(&bundle_serial_out);

        assert_eq!(decode_replay_bundle(&bundle_bytes).unwrap(), bundle_serial_out);
    }
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub mod rejection;
```

with:

```rust
pub mod rejection;
pub mod replay_serial;
```

In `shared/src/protocol/mod.rs`, replace:

```rust
pub use rejection::*;
```

with:

```rust
pub use rejection::*;
pub use replay_serial::*;
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib replay`

Expected: FAIL; the first error reads `` error[E0425]: cannot find type `ReplayHeaderSerialOut` in this scope ``.

- [ ] **Step 3: Implement**

Create `shared/src/replay/replay.rs`:

```rust
use crate::game::GameSettings;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayHeader {
    pub settings: GameSettings,
    pub seed: u64,
}
```

Create `shared/src/replay/mod.rs`:

```rust
pub mod replay;

pub use replay::*;
```

In `shared/src/lib.rs`, replace:

```rust
pub mod random;
```

with:

```rust
pub mod random;
pub mod replay;
```

In `shared/src/protocol/replay_serial.rs`, insert directly above `#[cfg(test)]`:

```rust
use bitcode::{Decode, Encode};

use crate::error::AppError;
use crate::game::GameSettings;
use crate::protocol::GameSettingsSerialOut;
use crate::replay::ReplayHeader;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ReplayHeaderSerialOut {
    pub settings: GameSettingsSerialOut,
    pub seed: u64,
}

impl From<&ReplayHeader> for ReplayHeaderSerialOut {
    fn from(header: &ReplayHeader) -> ReplayHeaderSerialOut {
        ReplayHeaderSerialOut {
            settings: GameSettingsSerialOut::from(&header.settings),
            seed: header.seed,
        }
    }
}

impl TryFrom<ReplayHeaderSerialOut> for ReplayHeader {
    type Error = AppError;

    fn try_from(header_serial_out: ReplayHeaderSerialOut) -> Result<ReplayHeader, AppError> {
        Ok(ReplayHeader {
            settings: GameSettings::try_from(header_serial_out.settings)?,
            seed: header_serial_out.seed,
        })
    }
}
```

In `shared/src/protocol/protocol.rs`, replace:

```rust
use bitcode::DecodeOwned;

use crate::error::AppError;
use crate::protocol::{GameStateSerialOut, MessageSerialIn, MessageSerialOut};
```

with:

```rust
use bitcode::DecodeOwned;

use crate::error::AppError;
use crate::protocol::{
    GameStateSerialOut, InputBundleSerialOut, MessageSerialIn, MessageSerialOut, ReplayHeaderSerialOut,
};
```

In `shared/src/protocol/protocol.rs`, insert directly above the line starting `fn decode<T: DecodeOwned>`:

```rust
pub fn encode_replay_header(header_serial_out: &ReplayHeaderSerialOut) -> Vec<u8> {
    bitcode::encode(header_serial_out)
}

pub fn decode_replay_header(bytes: &[u8]) -> Result<ReplayHeaderSerialOut, AppError> {
    decode(bytes, "replay header")
}

pub fn encode_replay_bundle(bundle_serial_out: &InputBundleSerialOut) -> Vec<u8> {
    bitcode::encode(bundle_serial_out)
}

pub fn decode_replay_bundle(bytes: &[u8]) -> Result<InputBundleSerialOut, AppError> {
    decode(bytes, "replay bundle")
}
```

- [ ] **Step 4: Run the test and see it pass**

Run: `cargo test -p shared --lib replay`

Expected: PASS, `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 312 filtered out; finished in 0.00s`

- [ ] **Step 5: Commit**

```sh
git add shared/src/replay/replay.rs shared/src/replay/mod.rs shared/src/protocol/replay_serial.rs shared/src/lib.rs shared/src/protocol/protocol.rs shared/src/protocol/mod.rs
git status
git commit -m "replay header"
```

### Task 19: Replay record stream

**Files:**
- Modify: `shared/src/replay/replay.rs`
- Test: inline `mod tests` in `shared/src/replay/replay.rs`

A replay file is `PROTOCOL_VERSION` as a little-endian `u16`, then length-prefixed records: the header, then one bundle per tick (protocol.md#wire-types). The length prefix is a little-endian `u32`, which the design leaves open. Another version is rejected before any record is decoded; a partial record is `Truncated`. The record functions are public for the server's incremental capture.

- [ ] **Step 1: Write the failing test**

Append to the end of `shared/src/replay/replay.rs`, after one blank line:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameModeKind, MemberEvent, Tick, test_fixture};
    use crate::member::{MemberId, MemberRoleKind};
    use crate::world::WorldShapeKind;

    fn create_replay_log() -> ReplayLog {
        ReplayLog {
            header: ReplayHeader {
                settings: test_fixture::create_settings(GameModeKind::FreeForAll, WorldShapeKind::Rectangle, 800),
                seed: 17,
            },
            input_bundles: vec![
                InputBundle {
                    tick: Tick(1),
                    member_events: vec![MemberEvent::Joined {
                        member_id: MemberId(0),
                        screen_name: String::from("Blob"),
                        role: MemberRoleKind::Spectator,
                        loadout: None,
                        team: None,
                    }],
                    player_inputs: Vec::new(),
                },
                InputBundle {
                    tick: Tick(2),
                    member_events: Vec::new(),
                    player_inputs: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn read_replay_restores_the_written_log() {
        let replay_log: ReplayLog = create_replay_log();

        assert_eq!(read_replay(&write_replay(&replay_log)).unwrap(), replay_log);
    }

    #[test]
    fn write_replay_starts_with_the_little_endian_protocol_version() {
        let replay_bytes: Vec<u8> = write_replay(&create_replay_log());

        assert_eq!(replay_bytes[..2], protocol::PROTOCOL_VERSION.to_le_bytes());
    }

    #[test]
    fn read_replay_rejects_another_protocol_version_before_decoding() {
        let mut replay_bytes: Vec<u8> = write_replay(&create_replay_log());
        replay_bytes[..2].copy_from_slice(&7_u16.to_le_bytes());
        replay_bytes.truncate(5);

        let replay_read_error: ReplayReadError = read_replay(&replay_bytes).unwrap_err();

        assert!(matches!(
            replay_read_error,
            ReplayReadError::ProtocolVersionMismatch {
                file_version: 7,
                current_version: protocol::PROTOCOL_VERSION,
            },
        ));
    }

    #[test]
    fn read_replay_rejects_a_truncated_record() {
        let replay_bytes: Vec<u8> = write_replay(&create_replay_log());

        let replay_read_error: ReplayReadError = read_replay(&replay_bytes[..replay_bytes.len() - 1]).unwrap_err();

        assert!(matches!(replay_read_error, ReplayReadError::Truncated));
        assert!(matches!(read_replay(&[1]).unwrap_err(), ReplayReadError::Truncated));
    }

    #[test]
    fn read_replay_rejects_a_file_without_a_header() {
        let replay_read_error: ReplayReadError = read_replay(&get_version_prefix()).unwrap_err();

        assert!(matches!(replay_read_error, ReplayReadError::MissingHeader));
    }

    #[test]
    fn read_replay_names_the_invalid_record() {
        let replay_log: ReplayLog = create_replay_log();
        let mut replay_bytes: Vec<u8> = get_version_prefix().to_vec();
        replay_bytes.extend(get_header_record_bytes(&ReplayHeaderSerialOut::from(
            &replay_log.header,
        )));
        replay_bytes.extend(get_length_prefixed_record(&[0xff, 0xff, 0xff]));

        let replay_read_error: ReplayReadError = read_replay(&replay_bytes).unwrap_err();

        assert!(matches!(
            replay_read_error,
            ReplayReadError::InvalidRecord { record_index: 1, .. }
        ));
    }
}
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib replay::`

Expected: FAIL; the first error reads `` error[E0425]: cannot find type `ReplayLog` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/replay/replay.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::error::AppError;
use crate::game::{GameSettings, InputBundle};
use crate::protocol;
use crate::protocol::{InputBundleSerialOut, ReplayHeaderSerialOut};

const VERSION_PREFIX_BYTE_COUNT: usize = 2;
const RECORD_LENGTH_BYTE_COUNT: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayHeader {
    pub settings: GameSettings,
    pub seed: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayLog {
    pub header: ReplayHeader,
    /// One per tick from tick 1.
    pub input_bundles: Vec<InputBundle>,
}

#[derive(Debug)]
pub enum ReplayReadError {
    ProtocolVersionMismatch {
        file_version: u16,
        current_version: u16,
    },
    Truncated,
    MissingHeader,
    /// Record 0 is the header.
    InvalidRecord {
        record_index: u32,
        error: AppError,
    },
}

/// Little-endian, ahead of the encoded records.
pub fn get_version_prefix() -> [u8; VERSION_PREFIX_BYTE_COUNT] {
    protocol::PROTOCOL_VERSION.to_le_bytes()
}

pub fn get_header_record_bytes(header_serial_out: &ReplayHeaderSerialOut) -> Vec<u8> {
    get_length_prefixed_record(&protocol::encode_replay_header(header_serial_out))
}

pub fn get_bundle_record_bytes(bundle_serial_out: &InputBundleSerialOut) -> Vec<u8> {
    get_length_prefixed_record(&protocol::encode_replay_bundle(bundle_serial_out))
}

pub fn write_replay(replay_log: &ReplayLog) -> Vec<u8> {
    let mut replay_bytes: Vec<u8> = get_version_prefix().to_vec();
    replay_bytes.extend(get_header_record_bytes(&ReplayHeaderSerialOut::from(
        &replay_log.header,
    )));

    for bundle in &replay_log.input_bundles {
        replay_bytes.extend(get_bundle_record_bytes(&InputBundleSerialOut::from(bundle)));
    }

    replay_bytes
}

pub fn read_replay(replay_bytes: &[u8]) -> Result<ReplayLog, ReplayReadError> {
    let Some((version_bytes, record_stream_bytes)): Option<(&[u8; VERSION_PREFIX_BYTE_COUNT], &[u8])> =
        replay_bytes.split_first_chunk::<VERSION_PREFIX_BYTE_COUNT>()
    else {
        return Err(ReplayReadError::Truncated);
    };

    let file_version: u16 = u16::from_le_bytes(*version_bytes);

    if file_version != protocol::PROTOCOL_VERSION {
        return Err(ReplayReadError::ProtocolVersionMismatch {
            file_version,
            current_version: protocol::PROTOCOL_VERSION,
        });
    }

    let records: Vec<&[u8]> = split_records(record_stream_bytes)?;
    let Some((header_record, bundle_records)): Option<(&&[u8], &[&[u8]])> = records.split_first() else {
        return Err(ReplayReadError::MissingHeader);
    };

    let header: ReplayHeader = read_header(header_record)?;
    let mut input_bundles: Vec<InputBundle> = Vec::new();

    for (bundle_record, record_index) in bundle_records.iter().zip(1_u32..) {
        let bundle: InputBundle =
            read_bundle(bundle_record).map_err(|error| ReplayReadError::InvalidRecord { record_index, error })?;
        input_bundles.push(bundle);
    }

    Ok(ReplayLog { header, input_bundles })
}

fn get_length_prefixed_record(record: &[u8]) -> Vec<u8> {
    let record_length: u32 = u32::try_from(record.len()).expect("a replay record is shorter than 4 GiB");
    let mut record_bytes: Vec<u8> = record_length.to_le_bytes().to_vec();
    record_bytes.extend_from_slice(record);

    record_bytes
}

fn split_records(record_stream_bytes: &[u8]) -> Result<Vec<&[u8]>, ReplayReadError> {
    let mut records: Vec<&[u8]> = Vec::new();
    let mut remaining_bytes: &[u8] = record_stream_bytes;

    while !remaining_bytes.is_empty() {
        let Some((length_bytes, after_length_bytes)): Option<(&[u8; RECORD_LENGTH_BYTE_COUNT], &[u8])> =
            remaining_bytes.split_first_chunk::<RECORD_LENGTH_BYTE_COUNT>()
        else {
            return Err(ReplayReadError::Truncated);
        };

        let record_length: usize =
            usize::try_from(u32::from_le_bytes(*length_bytes)).map_err(|_| ReplayReadError::Truncated)?;

        if after_length_bytes.len() < record_length {
            return Err(ReplayReadError::Truncated);
        }

        let (record, after_record_bytes): (&[u8], &[u8]) = after_length_bytes.split_at(record_length);
        records.push(record);
        remaining_bytes = after_record_bytes;
    }

    Ok(records)
}

fn read_header(header_record: &[u8]) -> Result<ReplayHeader, ReplayReadError> {
    protocol::decode_replay_header(header_record)
        .and_then(ReplayHeader::try_from)
        .map_err(|error| ReplayReadError::InvalidRecord { record_index: 0, error })
}

fn read_bundle(bundle_record: &[u8]) -> Result<InputBundle, AppError> {
    InputBundle::try_from(protocol::decode_replay_bundle(bundle_record)?)
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo test -p shared --lib replay:: && cargo test -p shared --test forbidden_operations`

Expected: PASS, one line per test binary:

```text
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 316 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
```

The forbidden-operations scan fails on the word bitcode anywhere in `shared/src` outside `protocol/`, comments included.

- [ ] **Step 5: Commit**

```sh
git add shared/src/replay/replay.rs
git status
git commit -m "replay record stream"
```

### Task 20: Running a replay

**Files:**
- Modify: `shared/src/replay/replay.rs`
- Test: inline `mod tests` in `shared/src/replay/replay.rs`

`run_replay` starts from `GameState::new(settings, seed)` and steps every bundle, as testing.md#replay-tests describes, giving the checksum after each tick. `format_tick_checksums` writes the phase 3 checksum file format (`<tick> <16 lowercase hexadecimal digits>`), shared by the replay tests and `protocol_dump replay --checksums`.

- [ ] **Step 1: Write the failing test**

In `shared/src/replay/replay.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn run_replay_gives_the_checksum_after_each_bundle() {
        let replay_log: ReplayLog = create_replay_log();
        let mut state: GameState = GameState::new(replay_log.header.settings.clone(), replay_log.header.seed);
        let mut expected_tick_checksums: Vec<TickChecksum> = Vec::new();

        for bundle in &replay_log.input_bundles {
            game::step(&mut state, bundle).unwrap();
            expected_tick_checksums.push(TickChecksum {
                tick: bundle.tick,
                checksum: state_checksum::get_state_checksum(&state),
            });
        }

        assert_eq!(run_replay(&replay_log).unwrap(), expected_tick_checksums);
    }

    #[test]
    fn run_replay_stops_at_a_bundle_out_of_order() {
        let mut replay_log: ReplayLog = create_replay_log();
        replay_log.input_bundles.swap(0, 1);

        assert_eq!(
            run_replay(&replay_log),
            Err(StepError::TickMismatch {
                expected: Tick(1),
                received: Tick(2),
            }),
        );
    }

    #[test]
    fn format_tick_checksums_writes_one_line_per_tick() {
        let tick_checksums: Vec<TickChecksum> = vec![
            TickChecksum {
                tick: Tick(1),
                checksum: 0xab,
            },
            TickChecksum {
                tick: Tick(2),
                checksum: 0xc6db_d9a8_567e_e3e6,
            },
        ];

        assert_eq!(
            format_tick_checksums(&tick_checksums),
            "1 00000000000000ab\n2 c6dbd9a8567ee3e6\n"
        );
    }
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --lib replay::`

Expected: FAIL; the first error reads `` error[E0425]: cannot find type `GameState` in this scope ``.

- [ ] **Step 3: Implement**

In `shared/src/replay/replay.rs`, replace:

```rust
use crate::error::AppError;
use crate::game::{GameSettings, InputBundle};
use crate::protocol;
use crate::protocol::{InputBundleSerialOut, ReplayHeaderSerialOut};
```

with:

```rust
use crate::error::AppError;
use crate::game;
use crate::game::{GameSettings, GameState, InputBundle, StepError, Tick};
use crate::protocol;
use crate::protocol::state_checksum;
use crate::protocol::{InputBundleSerialOut, ReplayHeaderSerialOut};
```

In `shared/src/replay/replay.rs`, insert directly above the line starting `/// Little-endian`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickChecksum {
    pub tick: Tick,
    /// Of the state after the tick.
    pub checksum: u64,
}
```

In `shared/src/replay/replay.rs`, insert directly above the line starting `fn get_length_prefixed_record(`:

```rust
/// From `GameState::new` with the header's settings and seed, one checksum per bundle.
pub fn run_replay(replay_log: &ReplayLog) -> Result<Vec<TickChecksum>, StepError> {
    let mut state: GameState = GameState::new(replay_log.header.settings.clone(), replay_log.header.seed);
    let mut tick_checksums: Vec<TickChecksum> = Vec::with_capacity(replay_log.input_bundles.len());

    for bundle in &replay_log.input_bundles {
        game::step(&mut state, bundle)?;

        tick_checksums.push(TickChecksum {
            tick: state.tick,
            checksum: state_checksum::get_state_checksum(&state),
        });
    }

    Ok(tick_checksums)
}

/// One line per tick: the tick, then the checksum as 16 lowercase hexadecimal digits.
pub fn format_tick_checksums(tick_checksums: &[TickChecksum]) -> String {
    tick_checksums
        .iter()
        .map(|tick_checksum| format!("{} {:016x}\n", tick_checksum.tick.0, tick_checksum.checksum))
        .collect()
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo test -p shared --lib replay:: && cargo test -p shared --test forbidden_operations`

Expected: PASS, one line per test binary:

```text
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 316 filtered out; finished in 0.00s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
```

- [ ] **Step 5: Commit**

```sh
git add shared/src/replay/replay.rs
git status
git commit -m "run replay"
```

### Task 21: protocol_dump frame

**Files:**
- Create: `tools/protocol_dump/src/frame_encoding.rs`
- Create: `tools/protocol_dump/src/frame_dump.rs`
- Modify: `tools/protocol_dump/src/main.rs`
- Test: inline `mod tests` in `tools/protocol_dump/src/frame_encoding.rs` and `tools/protocol_dump/src/frame_dump.rs`

`protocol_dump frame --direction client-to-server|server-to-client [--hex|--base64] [FILE]` decodes one frame from a file or standard input and prints the `Debug` form of the message (overview.md, `tools/protocol_dump`). Hex and base64 are decoded by hand (a few lines each) rather than through a new dependency, which would need the owner's approval. Errors print their chain of causes without minimer's backtraces and exit with status 1.

- [ ] **Step 1: Write the failing test**

Create `tools/protocol_dump/src/frame_encoding.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_frame_input_keeps_binary_input() {
        assert_eq!(
            decode_frame_input(&[0, 10, 255], FrameEncodingKind::Binary).unwrap(),
            vec![0, 10, 255]
        );
    }

    #[test]
    fn decode_frame_input_reads_hex_and_ignores_whitespace() {
        assert_eq!(
            decode_frame_input(b"00 0a\nFF", FrameEncodingKind::Hex).unwrap(),
            vec![0, 10, 255]
        );
    }

    #[test]
    fn decode_frame_input_rejects_odd_or_invalid_hex() {
        assert!(decode_frame_input(b"0a0", FrameEncodingKind::Hex).is_err());
        assert!(decode_frame_input(b"0g", FrameEncodingKind::Hex).is_err());
    }

    #[test]
    fn decode_frame_input_reads_base64_with_and_without_padding() {
        assert_eq!(
            decode_frame_input(b"Zm9vYmFy", FrameEncodingKind::Base64).unwrap(),
            b"foobar"
        );
        assert_eq!(
            decode_frame_input(b"Zm9vYg==\n", FrameEncodingKind::Base64).unwrap(),
            b"foob"
        );
        assert_eq!(
            decode_frame_input(b"Zm9vYmE", FrameEncodingKind::Base64).unwrap(),
            b"fooba"
        );
        assert_eq!(
            decode_frame_input(b"+/8=", FrameEncodingKind::Base64).unwrap(),
            vec![0xfb, 0xff]
        );
    }

    #[test]
    fn decode_frame_input_rejects_invalid_base64() {
        assert!(decode_frame_input(b"Zm9v!", FrameEncodingKind::Base64).is_err());
        assert!(decode_frame_input(b"Zm9vY", FrameEncodingKind::Base64).is_err());
    }
}
```

Create `tools/protocol_dump/src/frame_dump.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_from_reads_every_direction_name() {
        let directions: Vec<DirectionKind> =
            DIRECTION_NAMES.iter().map(|name| DirectionKind::try_from(*name).unwrap()).collect();

        assert_eq!(
            directions,
            vec![DirectionKind::ClientToServer, DirectionKind::ServerToClient]
        );
        assert!(DirectionKind::try_from("sideways").is_err());
    }

    #[test]
    fn dump_frame_prints_a_server_message() {
        let frame_bytes: Vec<u8> = protocol::encode_message_out(&MessageSerialOut::StateChecksum {
            tick: 14,
            checksum: 255,
        });

        assert_eq!(
            dump_frame(&frame_bytes, DirectionKind::ServerToClient).unwrap(),
            "StateChecksum {\n    tick: 14,\n    checksum: 255,\n}\n",
        );
    }

    #[test]
    fn dump_frame_prints_a_client_message() {
        let frame_bytes: Vec<u8> = protocol::encode_message_in(&MessageSerialIn::RequestSnapshot { client_tick: 3 });

        assert_eq!(
            dump_frame(&frame_bytes, DirectionKind::ClientToServer).unwrap(),
            "RequestSnapshot {\n    client_tick: 3,\n}\n",
        );
    }

    #[test]
    fn dump_frame_rejects_bytes_of_the_other_direction() {
        let frame_bytes: Vec<u8> = protocol::encode_message_out(&MessageSerialOut::JoinAccepted {
            game_id: 1,
            member_id: 2,
        });

        assert!(dump_frame(&frame_bytes, DirectionKind::ClientToServer).is_err());
    }
}
```

Replace the whole contents of `tools/protocol_dump/src/main.rs` with:

```rust
mod frame_dump;
mod frame_encoding;

fn main() {}
```

- [ ] **Step 2: Run the tests and see them fail**

Run: `cargo test -p protocol_dump`

Expected: FAIL; the first error reads `` error[E0425]: cannot find type `DirectionKind` in this scope ``.

- [ ] **Step 3: Implement**

In `tools/protocol_dump/src/frame_encoding.rs`, insert directly above `#[cfg(test)]`:

```rust
use shared::error::AppError;

const BASE64_PADDING: u8 = b'=';
const BASE64_BITS_PER_SYMBOL: u32 = 6;
const BITS_PER_BYTE: u32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameEncodingKind {
    Binary,
    Hex,
    Base64,
}

/// ASCII whitespace in hex and base64 input is ignored.
pub fn decode_frame_input(input_bytes: &[u8], encoding: FrameEncodingKind) -> Result<Vec<u8>, AppError> {
    match encoding {
        FrameEncodingKind::Binary => Ok(input_bytes.to_vec()),
        FrameEncodingKind::Hex => decode_hex(&get_symbols(input_bytes)),
        FrameEncodingKind::Base64 => decode_base64(&get_symbols(input_bytes)),
    }
}

fn get_symbols(input_bytes: &[u8]) -> Vec<u8> {
    input_bytes.iter().copied().filter(|byte| !byte.is_ascii_whitespace()).collect()
}

fn decode_hex(symbols: &[u8]) -> Result<Vec<u8>, AppError> {
    if symbols.len() % 2 != 0 {
        return Err(AppError::new("hex input has an odd number of digits"));
    }

    symbols
        .chunks(2)
        .map(|digit_pair| Ok(get_hex_value(digit_pair[0])? << 4 | get_hex_value(digit_pair[1])?))
        .collect()
}

fn get_hex_value(symbol: u8) -> Result<u8, AppError> {
    let Some(value): Option<u32> = char::from(symbol).to_digit(16) else {
        return Err(AppError::new(&format!("{:?} is not a hex digit", char::from(symbol))));
    };

    Ok(u8::try_from(value)?)
}

/// The standard alphabet; padding is optional.
fn decode_base64(symbols: &[u8]) -> Result<Vec<u8>, AppError> {
    let unpadded_symbols: &[u8] = symbols.strip_suffix(&[BASE64_PADDING, BASE64_PADDING]).unwrap_or(symbols);
    let unpadded_symbols: &[u8] = unpadded_symbols.strip_suffix(&[BASE64_PADDING]).unwrap_or(unpadded_symbols);

    if unpadded_symbols.len() % 4 == 1 {
        return Err(AppError::new("base64 input does not encode whole bytes"));
    }

    let mut bytes: Vec<u8> = Vec::new();
    let mut accumulated_bits: u32 = 0;
    let mut accumulated_bit_count: u32 = 0;

    for symbol in unpadded_symbols {
        accumulated_bits = (accumulated_bits << BASE64_BITS_PER_SYMBOL) | get_base64_value(*symbol)?;
        accumulated_bit_count += BASE64_BITS_PER_SYMBOL;

        if accumulated_bit_count >= BITS_PER_BYTE {
            accumulated_bit_count -= BITS_PER_BYTE;
            bytes.push(u8::try_from((accumulated_bits >> accumulated_bit_count) & 0xff)?);
            accumulated_bits &= (1 << accumulated_bit_count) - 1;
        }
    }

    Ok(bytes)
}

fn get_base64_value(symbol: u8) -> Result<u32, AppError> {
    let value: u8 = match symbol {
        b'A'..=b'Z' => symbol - b'A',
        b'a'..=b'z' => symbol - b'a' + 26,
        b'0'..=b'9' => symbol - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => {
            return Err(AppError::new(&format!(
                "{:?} is not a base64 symbol",
                char::from(symbol)
            )));
        }
    };

    Ok(u32::from(value))
}
```

In `tools/protocol_dump/src/frame_dump.rs`, insert directly above `#[cfg(test)]`:

```rust
use shared::error::AppError;
use shared::protocol;
use shared::protocol::{MessageSerialIn, MessageSerialOut};

const CLIENT_TO_SERVER_NAME: &str = "client-to-server";
const SERVER_TO_CLIENT_NAME: &str = "server-to-client";
pub const DIRECTION_NAMES: [&str; 2] = [CLIENT_TO_SERVER_NAME, SERVER_TO_CLIENT_NAME];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectionKind {
    ClientToServer,
    ServerToClient,
}

impl TryFrom<&str> for DirectionKind {
    type Error = AppError;

    fn try_from(name: &str) -> Result<DirectionKind, AppError> {
        match name {
            CLIENT_TO_SERVER_NAME => Ok(DirectionKind::ClientToServer),
            SERVER_TO_CLIENT_NAME => Ok(DirectionKind::ServerToClient),
            _ => Err(AppError::new(&format!("unknown direction {name:?}"))),
        }
    }
}

/// The `Debug` form of the decoded message.
pub fn dump_frame(frame_bytes: &[u8], direction: DirectionKind) -> Result<String, AppError> {
    let dump: String = match direction {
        DirectionKind::ClientToServer => {
            let message_serial_in: MessageSerialIn = protocol::decode_message_in(frame_bytes)?;

            format!("{message_serial_in:#?}\n")
        }
        DirectionKind::ServerToClient => {
            let message_serial_out: MessageSerialOut = protocol::decode_message_out(frame_bytes)?;

            format!("{message_serial_out:#?}\n")
        }
    };

    Ok(dump)
}
```

Replace the whole contents of `tools/protocol_dump/src/main.rs` with:

```rust
use std::error::Error;
use std::fs;
use std::io;
use std::io::Read;
use std::process::ExitCode;

use clap::{Arg, ArgAction, ArgMatches, Command};
use shared::error::AppError;

use crate::frame_dump::DirectionKind;
use crate::frame_encoding::FrameEncodingKind;

mod frame_dump;
mod frame_encoding;

fn main() -> ExitCode {
    env_logger::init();

    let matches: ArgMatches = create_command().get_matches();
    let dump_result: Result<String, AppError> = run_subcommand(&matches);

    match dump_result {
        Ok(dump) => {
            print!("{dump}");

            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", get_error_text(&error));

            ExitCode::FAILURE
        }
    }
}

fn create_command() -> Command {
    Command::new("protocol_dump")
        .about("Decodes captured frames and replay files")
        .subcommand_required(true)
        .subcommand(
            Command::new("frame")
                .about("Decodes one frame and prints its Debug form")
                .arg(Arg::new("direction").long("direction").required(true).value_parser(frame_dump::DIRECTION_NAMES))
                .arg(
                    Arg::new("hex")
                        .long("hex")
                        .action(ArgAction::SetTrue)
                        .conflicts_with("base64")
                        .help("The frame is written as hexadecimal digits"),
                )
                .arg(
                    Arg::new("base64")
                        .long("base64")
                        .action(ArgAction::SetTrue)
                        .help("The frame is written as base64, as copied from browser devtools"),
                )
                .arg(Arg::new("file").value_name("FILE").help("Read from standard input when absent")),
        )
}

fn run_subcommand(matches: &ArgMatches) -> Result<String, AppError> {
    match matches.subcommand() {
        Some(("frame", frame_matches)) => run_frame(frame_matches),
        Some((other, _)) => Err(AppError::new(&format!("unknown subcommand {other}"))),
        None => Err(AppError::new("missing subcommand")),
    }
}

fn run_frame(frame_matches: &ArgMatches) -> Result<String, AppError> {
    let direction_name: &String = frame_matches.get_one::<String>("direction").expect("direction is required via clap");
    let direction: DirectionKind = DirectionKind::try_from(direction_name.as_str())?;
    let encoding: FrameEncodingKind = get_frame_encoding(frame_matches);
    let input_bytes: Vec<u8> = read_input(frame_matches.get_one::<String>("file"))?;
    let frame_bytes: Vec<u8> = frame_encoding::decode_frame_input(&input_bytes, encoding)?;

    frame_dump::dump_frame(&frame_bytes, direction)
}

fn get_frame_encoding(frame_matches: &ArgMatches) -> FrameEncodingKind {
    if frame_matches.get_flag("hex") {
        return FrameEncodingKind::Hex;
    }

    if frame_matches.get_flag("base64") {
        return FrameEncodingKind::Base64;
    }

    FrameEncodingKind::Binary
}

fn read_input(file_path: Option<&String>) -> Result<Vec<u8>, AppError> {
    let Some(file_path): Option<&String> = file_path else {
        let mut input_bytes: Vec<u8> = Vec::new();
        io::stdin().read_to_end(&mut input_bytes)?;

        return Ok(input_bytes);
    };

    Ok(fs::read(file_path)?)
}

/// The message and its chain of causes, without backtraces.
fn get_error_text(error: &AppError) -> String {
    let Some(sub_error): Option<&Box<dyn Error>> = error.sub_error.as_ref() else {
        return error.message.clone();
    };

    let sub_error_text: String = match sub_error.downcast_ref::<AppError>() {
        Some(sub_app_error) => get_error_text(sub_app_error),
        None => sub_error.to_string(),
    };

    format!("{}: {sub_error_text}", error.message)
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo test -p protocol_dump`

Expected: PASS, `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`

- [ ] **Step 5: Decode a frame written as hex**

Run: `echo '04 04 0e 06 ff' | cargo run -q -p protocol_dump -- frame --direction server-to-client --hex`

Expected output:

```text
StateChecksum {
    tick: 14,
    checksum: 255,
}
```

- [ ] **Step 6: Decode the same frame written as base64**

Run: `echo 'BAQOBv8=' | cargo run -q -p protocol_dump -- frame --direction server-to-client --base64`

Expected output:

```text
StateChecksum {
    tick: 14,
    checksum: 255,
}
```

- [ ] **Step 7: See a malformed frame rejected**

Run: `echo '05' | cargo run -q -p protocol_dump -- frame --direction server-to-client --hex`

Expected: exit status 1, printing `Error: cannot decode the server message: EOF`.

- [ ] **Step 8: Commit**

```sh
git add tools/protocol_dump/src/frame_encoding.rs tools/protocol_dump/src/frame_dump.rs tools/protocol_dump/src/main.rs
git status
git commit -m "protocol dump frame"
```

### Task 22: protocol_dump replay

**Files:**
- Create: `tools/protocol_dump/src/replay_dump.rs`
- Modify: `tools/protocol_dump/src/main.rs`
- Test: inline `mod tests` in `tools/protocol_dump/src/replay_dump.rs`

`protocol_dump replay FILE [--checksums]` reads a replay; without the flag it prints the header and every bundle, with it only the checksum lines of a re-run, so the regeneration script can redirect them into a `.checksums` file (dev-workflow.md#scripts).

- [ ] **Step 1: Write the failing test**

Create `tools/protocol_dump/src/replay_dump.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::game::{GameModeKind, GameSettings, InputBundle, Tick};
    use shared::replay::ReplayHeader;
    use shared::world::WorldShapeKind;

    fn create_replay_bytes() -> Vec<u8> {
        let replay_log: ReplayLog = ReplayLog {
            header: ReplayHeader {
                settings: GameSettings {
                    title: String::from("Dumped game"),
                    mode: GameModeKind::FreeForAll,
                    world_shape: WorldShapeKind::Rectangle,
                    world_width_pixels: 800,
                    world_height_pixels: 800,
                    player_minimum: None,
                    player_cap: 8,
                    team_count: None,
                    leaderboard_length: 10,
                },
                seed: 5,
            },
            input_bundles: (1..=3)
                .map(|tick| InputBundle {
                    tick: Tick(tick),
                    member_events: Vec::new(),
                    player_inputs: Vec::new(),
                })
                .collect(),
        };

        replay::write_replay(&replay_log)
    }

    #[test]
    fn dump_replay_prints_the_checksum_lines_of_a_rerun() {
        let replay_bytes: Vec<u8> = create_replay_bytes();
        let replay_log: ReplayLog = replay::read_replay(&replay_bytes).unwrap();
        let expected_dump: String = replay::format_tick_checksums(&replay::run_replay(&replay_log).unwrap());

        assert_eq!(
            dump_replay(&replay_bytes, ReplayOutputKind::Checksums).unwrap(),
            expected_dump
        );
    }

    #[test]
    fn dump_replay_prints_the_header_then_every_bundle() {
        let dump: String = dump_replay(&create_replay_bytes(), ReplayOutputKind::Bundles).unwrap();

        assert!(dump.starts_with("ReplayHeader {\n"));
        assert_eq!(dump.matches("InputBundle {\n").count(), 3);
    }

    #[test]
    fn dump_replay_names_another_protocol_version() {
        let mut replay_bytes: Vec<u8> = create_replay_bytes();
        replay_bytes[..2].copy_from_slice(&9_u16.to_le_bytes());

        let error: AppError = dump_replay(&replay_bytes, ReplayOutputKind::Bundles).unwrap_err();

        assert_eq!(error.message, "Error: the replay has protocol version 9, not 1");
    }
}
```

In `tools/protocol_dump/src/main.rs`, replace:

```rust
mod frame_encoding;
```

with:

```rust
mod frame_encoding;
mod replay_dump;
```

- [ ] **Step 2: Run the tests and see them fail**

Run: `cargo test -p protocol_dump`

Expected: FAIL; the first error reads `` error[E0425]: cannot find type `ReplayLog` in this scope ``.

- [ ] **Step 3: Implement**

In `tools/protocol_dump/src/replay_dump.rs`, insert directly above `#[cfg(test)]`:

```rust
use shared::error::AppError;
use shared::replay;
use shared::replay::{ReplayLog, ReplayReadError, TickChecksum};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayOutputKind {
    /// The header and every bundle, in their `Debug` form.
    Bundles,
    /// The checksum after each tick of a re-run.
    Checksums,
}

pub fn dump_replay(replay_bytes: &[u8], output: ReplayOutputKind) -> Result<String, AppError> {
    let replay_log: ReplayLog = replay::read_replay(replay_bytes).map_err(get_replay_read_error)?;

    match output {
        ReplayOutputKind::Bundles => Ok(format_replay_log(&replay_log)),
        ReplayOutputKind::Checksums => {
            let tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log)
                .map_err(|step_error| AppError::new(&format!("the replay does not step: {step_error:?}")))?;

            Ok(replay::format_tick_checksums(&tick_checksums))
        }
    }
}

fn format_replay_log(replay_log: &ReplayLog) -> String {
    let mut dump: String = format!("{:#?}\n", replay_log.header);

    for bundle in &replay_log.input_bundles {
        dump.push_str(&format!("{bundle:#?}\n"));
    }

    dump
}

fn get_replay_read_error(replay_read_error: ReplayReadError) -> AppError {
    match replay_read_error {
        ReplayReadError::ProtocolVersionMismatch {
            file_version,
            current_version,
        } => AppError::new(&format!(
            "the replay has protocol version {file_version}, not {current_version}"
        )),
        ReplayReadError::Truncated => AppError::new("the replay is truncated"),
        ReplayReadError::MissingHeader => AppError::new("the replay has no header"),
        ReplayReadError::InvalidRecord { record_index, error } => {
            AppError::from_error(&format!("replay record {record_index} is invalid"), Box::new(error))
        }
    }
}
```

In `tools/protocol_dump/src/main.rs`, replace:

```rust
use crate::frame_encoding::FrameEncodingKind;
```

with:

```rust
use crate::frame_encoding::FrameEncodingKind;
use crate::replay_dump::ReplayOutputKind;
```

In `tools/protocol_dump/src/main.rs`, replace:

```rust
                .arg(Arg::new("file").value_name("FILE").help("Read from standard input when absent")),
        )
}
```

with:

```rust
                .arg(Arg::new("file").value_name("FILE").help("Read from standard input when absent")),
        )
        .subcommand(
            Command::new("replay")
                .about("Decodes a replay file and prints its bundles")
                .arg(Arg::new("file").value_name("FILE").required(true))
                .arg(
                    Arg::new("checksums")
                        .long("checksums")
                        .action(ArgAction::SetTrue)
                        .help("Re-runs the replay and prints only the checksum after each tick"),
                ),
        )
}
```

In `tools/protocol_dump/src/main.rs`, replace:

```rust
        Some(("frame", frame_matches)) => run_frame(frame_matches),
```

with:

```rust
        Some(("frame", frame_matches)) => run_frame(frame_matches),
        Some(("replay", replay_matches)) => run_replay(replay_matches),
```

In `tools/protocol_dump/src/main.rs`, insert directly above the line starting `fn get_frame_encoding(`:

```rust
fn run_replay(replay_matches: &ArgMatches) -> Result<String, AppError> {
    let file_path: &String = replay_matches.get_one::<String>("file").expect("file is required via clap");
    let output: ReplayOutputKind = if replay_matches.get_flag("checksums") {
        ReplayOutputKind::Checksums
    } else {
        ReplayOutputKind::Bundles
    };
    let replay_bytes: Vec<u8> = fs::read(file_path)?;

    replay_dump::dump_replay(&replay_bytes, output)
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo test -p protocol_dump`

Expected: PASS, `test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s`

- [ ] **Step 5: See a replay of another protocol version rejected**

Run: `printf '\011\000' > /tmp/other_version.replay && cargo run -q -p protocol_dump -- replay /tmp/other_version.replay`

Expected: exit status 1, printing `Error: the replay has protocol version 9, not 1`.

- [ ] **Step 6: Commit**

```sh
git add tools/protocol_dump/src/replay_dump.rs tools/protocol_dump/src/main.rs
git status
git commit -m "protocol dump replay"
```

### Task 23: Scripted game replay fixture

**Files:**
- Create: `shared/tests/replay.rs`
- Create: `shared/tests/fixtures/scripted_game.replay`
- Modify: `shared/tests/determinism.rs`
- Modify: `scripts/test/regenerate-replay-fixtures.sh`
- Test: `shared/tests/replay.rs`

The phase 3 scripted players move from `determinism.rs` into `shared/tests/replay.rs`, whose ignored `regenerate_scripted_game_fixture` writes `scripted_game.replay` (testing.md#replay-tests). The fixture test re-runs every `fixtures/*.replay` against its `.checksums` file. The replayed game is the scripted game, so the committed checksums of phase 3 stay byte-identical; `determinism.rs` keeps the two-run test, now over the fixture.

- [ ] **Step 1: Write the failing test**

Create `shared/tests/replay.rs`:

```rust
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use shared::ability::{AbilityPressSet, AimVector, FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use shared::game;
use shared::game::{GameModeKind, GameSettings, GameState, InputBundle, MemberEvent, PlayerTickInput, Tick};
use shared::geometry::WorldPoint;
use shared::member::{Appearance, MemberId, MemberRoleKind, OrganismColorKind, SkinKind};
use shared::random::Pcg32;
use shared::replay;
use shared::replay::{ReplayHeader, ReplayLog, TickChecksum};
use shared::world::WorldShapeKind;

const REPLAY_EXTENSION: &str = "replay";
const CHECKSUMS_EXTENSION: &str = "checksums";
const SCRIPTED_GAME_FIXTURE_NAME: &str = "scripted_game";
const SCRIPTED_TICK_COUNT: u32 = 10_000;
const SCRIPTED_PLAYER_COUNT: u32 = 16;
const SCRIPTED_PLAYER_MINIMUM: u8 = 4;
const SCRIPTED_LEADERBOARD_LENGTH: u8 = 10;
const GAME_SEED: u64 = 0x5c41_97ed_0000_0001;
const SCRIPT_SEED: u64 = 0x5c41_97ed_0000_0002;
const WORLD_SIZE_PIXELS: u32 = 800;
const CURSOR_TARGET_MARGIN_PIXELS: u32 = 100;
const CURSOR_STEP_PIXELS: i32 = 3;
const PRESS_CHANCE_DENOMINATOR: u32 = 32;
const AIM_EXTENT_PIXELS: u32 = 100;
const JOIN_TICK: Tick = Tick(1);
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

    /// Includes the zero aim, which launches no shot.
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
        player_minimum: Some(SCRIPTED_PLAYER_MINIMUM),
        player_cap: u8::try_from(SCRIPTED_PLAYER_COUNT).unwrap(),
        team_count: None,
        leaderboard_length: SCRIPTED_LEADERBOARD_LENGTH,
    }
}

/// Bit 0 picks the first ability, bit 1 the second, bit 2 the third.
fn create_loadout(player_index: u32) -> Loadout {
    let appearance_count: u32 = u32::try_from(COLORS.len()).unwrap();
    let appearance_index: usize = usize::try_from(player_index % appearance_count).unwrap();

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
        JOIN_TICK => {
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

fn create_scripted_game_log() -> ReplayLog {
    let header: ReplayHeader = ReplayHeader {
        settings: create_scripted_settings(),
        seed: GAME_SEED,
    };
    let mut state: GameState = GameState::new(header.settings.clone(), header.seed);
    let mut scripted_players: ScriptedPlayers = ScriptedPlayers::new();
    let mut input_bundles: Vec<InputBundle> = Vec::new();

    for _ in 0..SCRIPTED_TICK_COUNT {
        let bundle: InputBundle = scripted_players.create_bundle(&state);

        game::step(&mut state, &bundle).unwrap();

        input_bundles.push(bundle);
    }

    ReplayLog { header, input_bundles }
}

fn get_fixture_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn get_replay_fixture_paths() -> Vec<PathBuf> {
    let mut replay_fixture_paths: Vec<PathBuf> = fs::read_dir(get_fixture_directory())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == REPLAY_EXTENSION))
        .collect();
    replay_fixture_paths.sort();

    replay_fixture_paths
}

#[test]
fn every_replay_fixture_matches_its_checksums() {
    let replay_fixture_paths: Vec<PathBuf> = get_replay_fixture_paths();

    assert!(!replay_fixture_paths.is_empty());

    for replay_fixture_path in replay_fixture_paths {
        let replay_log: ReplayLog = replay::read_replay(&fs::read(&replay_fixture_path).unwrap()).unwrap();
        let tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log).unwrap();
        let golden_checksums: String =
            fs::read_to_string(replay_fixture_path.with_extension(CHECKSUMS_EXTENSION)).unwrap();

        assert_eq!(replay::format_tick_checksums(&tick_checksums), golden_checksums);
    }
}

#[test]
#[ignore = "rewrites the scripted game fixture; ./scripts/test/regenerate-replay-fixtures.sh"]
fn regenerate_scripted_game_fixture() {
    let fixture_path: PathBuf =
        get_fixture_directory().join(SCRIPTED_GAME_FIXTURE_NAME).with_extension(REPLAY_EXTENSION);

    fs::create_dir_all(get_fixture_directory()).unwrap();
    fs::write(fixture_path, replay::write_replay(&create_scripted_game_log())).unwrap();
}
```

- [ ] **Step 2: Run the test and see it fail**

Run: `cargo test -p shared --test replay`

Expected: FAIL; the test panics with `assertion failed: !replay_fixture_paths.is_empty()`.

No `.replay` fixture exists yet.

- [ ] **Step 3: Generate the fixture**

Run: `cargo test -p shared --test replay -- --ignored --exact regenerate_scripted_game_fixture`

Expected: PASS, `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 6.15s`

- [ ] **Step 4: Check the fixture bytes**

Run: `shasum -a 256 shared/tests/fixtures/scripted_game.replay`

Expected output:

```text
4c20a5a8f0eb1f218015aff4f106d54151eba4d0d07f91643e18a5806e191891  shared/tests/fixtures/scripted_game.replay
```

- [ ] **Step 5: Run the test and see it pass**

Run: `cargo test -p shared --test replay`

Expected: PASS, `test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 6.55s`

- [ ] **Step 6: Reduce the determinism test to the fixture**

Replace the whole contents of `shared/tests/determinism.rs` with:

```rust
use std::fs;
use std::path::{Path, PathBuf};

use shared::replay;
use shared::replay::{ReplayLog, TickChecksum};

fn read_scripted_game_log() -> ReplayLog {
    let fixture_path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scripted_game.replay");

    replay::read_replay(&fs::read(fixture_path).unwrap()).unwrap()
}

#[test]
fn run_replay_gives_identical_checksums_in_two_runs() {
    let replay_log: ReplayLog = read_scripted_game_log();
    let first_tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log).unwrap();
    let second_tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log).unwrap();

    assert_eq!(first_tick_checksums.len(), 10_000);
    assert_eq!(first_tick_checksums, second_tick_checksums);
}
```

- [ ] **Step 7: Run the determinism test**

Run: `cargo test -p shared --test determinism`

Expected: PASS, `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.03s`

- [ ] **Step 8: Regenerate checksums through protocol_dump**

Replace the whole contents of `scripts/test/regenerate-replay-fixtures.sh` with:

```sh
#!/usr/bin/env bash
# Rebuilds the scripted game fixture, then rewrites every fixture's golden checksums from the current simulation.

set -euo pipefail

readonly FIXTURE_DIRECTORY="shared/tests/fixtures"

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

cargo test -p shared --test replay -- --ignored --exact regenerate_scripted_game_fixture
cargo build -p protocol_dump

for replay_path in "${FIXTURE_DIRECTORY}"/*.replay; do
    checksums_path="${replay_path%.replay}.checksums"
    cargo run -q -p protocol_dump -- replay --checksums "${replay_path}" > "${checksums_path}"
    echo "wrote ${checksums_path}"
done
```

- [ ] **Step 9: Run the regeneration script and see the checksums unchanged**

Run: `./scripts/test/regenerate-replay-fixtures.sh && git diff --exit-code --stat -- shared/tests/fixtures/scripted_game.checksums && shasum -a 256 shared/tests/fixtures/scripted_game.replay`

Expected: the output ends with:

```text
wrote shared/tests/fixtures/scripted_game.checksums
4c20a5a8f0eb1f218015aff4f106d54151eba4d0d07f91643e18a5806e191891  shared/tests/fixtures/scripted_game.replay
```

`git diff --exit-code` prints nothing and succeeds: the checksums of phase 3 are unchanged.

- [ ] **Step 10: Commit**

```sh
git add shared/tests/replay.rs shared/tests/fixtures/scripted_game.replay shared/tests/determinism.rs scripts/test/regenerate-replay-fixtures.sh
git status
git commit -m "scripted game replay fixture"
```

### Task 24: Snapshot round trip

**Files:**
- Modify: `shared/tests/determinism.rs`
- Test: `shared/tests/determinism.rs`

At tick 2500 of the scripted game the state goes through `GameStateSerialOut::from`, `encode_game_state`, `decode_game_state` and `GameState::try_from`; both copies then step the remaining 7500 bundles with equal checksums (testing.md#determinism-tests).

- [ ] **Step 1: Write the test**

Replace the whole contents of `shared/tests/determinism.rs` with:

```rust
use std::fs;
use std::path::{Path, PathBuf};

use shared::game;
use shared::game::{GameState, Tick};
use shared::protocol;
use shared::protocol::GameStateSerialOut;
use shared::protocol::state_checksum;
use shared::replay;
use shared::replay::{ReplayLog, TickChecksum};

const SNAPSHOT_TICK: Tick = Tick(2500);

fn read_scripted_game_log() -> ReplayLog {
    let fixture_path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scripted_game.replay");

    replay::read_replay(&fs::read(fixture_path).unwrap()).unwrap()
}

fn restore_from_snapshot(state: &GameState) -> GameState {
    let snapshot_bytes: Vec<u8> = protocol::encode_game_state(&GameStateSerialOut::from(state));

    GameState::try_from(protocol::decode_game_state(&snapshot_bytes).unwrap()).unwrap()
}

#[test]
fn run_replay_gives_identical_checksums_in_two_runs() {
    let replay_log: ReplayLog = read_scripted_game_log();
    let first_tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log).unwrap();
    let second_tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log).unwrap();

    assert_eq!(first_tick_checksums.len(), 10_000);
    assert_eq!(first_tick_checksums, second_tick_checksums);
}

#[test]
fn snapshot_round_trip_continues_with_equal_checksums() {
    let replay_log: ReplayLog = read_scripted_game_log();
    let mut state: GameState = GameState::new(replay_log.header.settings.clone(), replay_log.header.seed);
    let mut restored_state: Option<GameState> = None;

    for bundle in &replay_log.input_bundles {
        game::step(&mut state, bundle).unwrap();

        if let Some(restored_state) = restored_state.as_mut() {
            game::step(restored_state, bundle).unwrap();

            assert_eq!(
                state_checksum::get_state_checksum(restored_state),
                state_checksum::get_state_checksum(&state)
            );
        }

        if bundle.tick == SNAPSHOT_TICK {
            let snapshot_state: GameState = restore_from_snapshot(&state);

            assert_eq!(snapshot_state, state);

            restored_state = Some(snapshot_state);
        }
    }

    assert!(restored_state.is_some());
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p shared --test determinism`

Expected: PASS, `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.38s`

It passes at once: the conversions it exercises exist since Tasks 12 and 17, and from here on it guards them.

- [ ] **Step 3: Commit**

```sh
git add shared/tests/determinism.rs
git status
git commit -m "snapshot round trip test"
```

### Task 25: Model to wire to model over the scripted game

**Files:**
- Create: `shared/tests/protocol.rs`
- Create: `shared/tests/helpers/mod.rs`
- Create: `shared/tests/helpers/replay_fixture.rs`
- Modify: `shared/tests/determinism.rs`
- Test: `shared/tests/protocol.rs`

Every bundle and the state after every tick of the scripted game convert to their wire types and back to equal models (testing.md#protocol-tests), and the run is checked to have reached every ability, spore, shot and round phase, field centres and both received effects. The fixture reader moves to `tests/helpers/` now that a second test file reads it.

- [ ] **Step 1: Lift the fixture reader**

Create `shared/tests/helpers/mod.rs`:

```rust
pub mod replay_fixture;
```

Create `shared/tests/helpers/replay_fixture.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

use shared::replay;
use shared::replay::ReplayLog;

pub fn read_scripted_game_log() -> ReplayLog {
    let fixture_path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scripted_game.replay");

    replay::read_replay(&fs::read(fixture_path).unwrap()).unwrap()
}
```

Replace the whole contents of `shared/tests/determinism.rs` with:

```rust
mod helpers;

use shared::game;
use shared::game::{GameState, Tick};
use shared::protocol;
use shared::protocol::GameStateSerialOut;
use shared::protocol::state_checksum;
use shared::replay;
use shared::replay::{ReplayLog, TickChecksum};

use crate::helpers::replay_fixture;

const SNAPSHOT_TICK: Tick = Tick(2500);

fn restore_from_snapshot(state: &GameState) -> GameState {
    let snapshot_bytes: Vec<u8> = protocol::encode_game_state(&GameStateSerialOut::from(state));

    GameState::try_from(protocol::decode_game_state(&snapshot_bytes).unwrap()).unwrap()
}

#[test]
fn run_replay_gives_identical_checksums_in_two_runs() {
    let replay_log: ReplayLog = replay_fixture::read_scripted_game_log();
    let first_tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log).unwrap();
    let second_tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log).unwrap();

    assert_eq!(first_tick_checksums.len(), 10_000);
    assert_eq!(first_tick_checksums, second_tick_checksums);
}

#[test]
fn snapshot_round_trip_continues_with_equal_checksums() {
    let replay_log: ReplayLog = replay_fixture::read_scripted_game_log();
    let mut state: GameState = GameState::new(replay_log.header.settings.clone(), replay_log.header.seed);
    let mut restored_state: Option<GameState> = None;

    for bundle in &replay_log.input_bundles {
        game::step(&mut state, bundle).unwrap();

        if let Some(restored_state) = restored_state.as_mut() {
            game::step(restored_state, bundle).unwrap();

            assert_eq!(
                state_checksum::get_state_checksum(restored_state),
                state_checksum::get_state_checksum(&state)
            );
        }

        if bundle.tick == SNAPSHOT_TICK {
            let snapshot_state: GameState = restore_from_snapshot(&state);

            assert_eq!(snapshot_state, state);

            restored_state = Some(snapshot_state);
        }
    }

    assert!(restored_state.is_some());
}
```

- [ ] **Step 2: Write the test**

Create `shared/tests/protocol.rs`:

```rust
mod helpers;

use std::collections::BTreeSet;

use shared::ability::{AbilityPhase, OrganismAbilities, ShotPhase, SporePhase};
use shared::game;
use shared::game::{GameState, InputBundle};
use shared::organism::Organism;
use shared::protocol::{GameStateSerialOut, InputBundleSerialOut};
use shared::replay::ReplayLog;
use shared::round::RoundPhase;

use crate::helpers::replay_fixture;

const EXPECTED_PHASE_NAMES: [&str; 18] = [
    "ability active",
    "ability cooling",
    "ability ready",
    "compressed",
    "field center",
    "frozen",
    "round playing",
    "round post-round",
    "round pre-round",
    "round waiting",
    "shot cooling",
    "shot flying",
    "shot ready",
    "shot secreting",
    "spore cooling",
    "spore flying",
    "spore ready",
    "spore secreting",
];

fn get_ability_phase_name(ability_phase: &AbilityPhase) -> &'static str {
    match ability_phase {
        AbilityPhase::Ready => "ability ready",
        AbilityPhase::Active { .. } => "ability active",
        AbilityPhase::Cooling { .. } => "ability cooling",
    }
}

fn get_spore_phase_name(spore_phase: &SporePhase) -> &'static str {
    match spore_phase {
        SporePhase::Ready => "spore ready",
        SporePhase::Flying { .. } => "spore flying",
        SporePhase::Secreting { .. } => "spore secreting",
        SporePhase::Cooling { .. } => "spore cooling",
    }
}

fn get_shot_phase_name(shot_phase: &ShotPhase) -> &'static str {
    match shot_phase {
        ShotPhase::Ready => "shot ready",
        ShotPhase::Flying { .. } => "shot flying",
        ShotPhase::Secreting { .. } => "shot secreting",
        ShotPhase::Cooling { .. } => "shot cooling",
    }
}

fn get_round_phase_name(round_phase: &RoundPhase) -> &'static str {
    match round_phase {
        RoundPhase::Waiting => "round waiting",
        RoundPhase::PreRound => "round pre-round",
        RoundPhase::Playing => "round playing",
        RoundPhase::PostRound => "round post-round",
    }
}

fn get_ability_names(abilities: &OrganismAbilities) -> Vec<&'static str> {
    let mut phase_names: Vec<&'static str> = vec![
        get_ability_phase_name(&abilities.first),
        get_ability_phase_name(&abilities.second),
        get_ability_phase_name(&abilities.third),
        get_spore_phase_name(&abilities.spore),
        get_shot_phase_name(&abilities.shots[0]),
        get_shot_phase_name(&abilities.shots[1]),
    ];

    if abilities.third_center.is_some() {
        phase_names.push("field center");
    }

    if abilities.compressed_until.is_some() {
        phase_names.push("compressed");
    }

    if abilities.frozen_until.is_some() {
        phase_names.push("frozen");
    }

    phase_names
}

fn get_phase_names(state: &GameState) -> Vec<&'static str> {
    let mut phase_names: Vec<&'static str> =
        state.round.iter().map(|round| get_round_phase_name(&round.phase)).collect();

    for member in state.members.values() {
        let Some(organism): Option<&Organism> = member.organism.as_ref() else {
            continue;
        };

        phase_names.extend(get_ability_names(&organism.abilities));
    }

    phase_names
}

#[test]
fn try_from_restores_every_scripted_state_and_bundle() {
    let replay_log: ReplayLog = replay_fixture::read_scripted_game_log();
    let mut state: GameState = GameState::new(replay_log.header.settings.clone(), replay_log.header.seed);
    let mut observed_phase_names: BTreeSet<&'static str> = BTreeSet::new();

    for bundle in &replay_log.input_bundles {
        assert_eq!(
            &InputBundle::try_from(InputBundleSerialOut::from(bundle)).unwrap(),
            bundle
        );

        game::step(&mut state, bundle).unwrap();

        assert_eq!(GameState::try_from(GameStateSerialOut::from(&state)).unwrap(), state);

        observed_phase_names.extend(get_phase_names(&state));
    }

    assert_eq!(observed_phase_names, BTreeSet::from(EXPECTED_PHASE_NAMES));
}
```

- [ ] **Step 3: Run it**

Run: `cargo test -p shared --test protocol && cargo test -p shared --test determinism`

Expected: PASS, one line per test binary:

```text
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.59s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.26s
```

It passes at once: it checks the conversions of Tasks 9 to 13 against real game states.

- [ ] **Step 4: Commit**

```sh
git add shared/tests/protocol.rs shared/tests/helpers/mod.rs shared/tests/helpers/replay_fixture.rs shared/tests/determinism.rs
git status
git commit -m "model to wire to model over the scripted game"
```

### Task 26: Malformed input

**Files:**
- Modify: `shared/tests/protocol.rs`
- Test: `shared/tests/protocol.rs`

Truncated, bit-flipped, extended and random frames, from 400 seeds, of five server and five client messages, the server ones taken from the scripted game at tick 2500. Decoding returns `Err` or a value, never panics; every decoded value goes through its `TryFrom`, which also never panics. The test counts all three outcomes so it fails if the mutations stop reaching any of them. A truncated or flipped replay is read the same way.

- [ ] **Step 1: Write the test**

In `shared/tests/protocol.rs`, replace:

```rust
mod helpers;

use std::collections::BTreeSet;

use shared::ability::{AbilityPhase, OrganismAbilities, ShotPhase, SporePhase};
use shared::game;
use shared::game::{GameState, InputBundle};
use shared::organism::Organism;
use shared::protocol::{GameStateSerialOut, InputBundleSerialOut};
use shared::replay::ReplayLog;
use shared::round::RoundPhase;

use crate::helpers::replay_fixture;
```

with:

```rust
mod helpers;

use std::collections::BTreeSet;

use shared::ability::{AbilityPhase, OrganismAbilities, ShotPhase, SporePhase};
use shared::error::AppError;
use shared::game;
use shared::game::{GameSettings, GameState, GameSummary, InputBundle, PlayerInput, Tick};
use shared::geometry::SubpixelPoint;
use shared::member::Joiner;
use shared::organism::Organism;
use shared::protocol;
use shared::protocol::protocol_limits;
use shared::protocol::{
    AimVectorSerial, AppearanceSerial, FirstAbilityKindSerial, GameModeKindSerial, GameSettingsSerialIn,
    GameSnapshotSerialOut, GameStateSerialOut, GameSummarySerialOut, InputBundleSerialOut, JoinerSerialIn,
    LoadoutSerial, MessageSerialIn, MessageSerialOut, OrganismColorKindSerial, PlayerInputSerialIn, RejectionKind,
    RejectionKindSerialOut, RequestKindSerialOut, SecondAbilityKindSerial, SkinKindSerial, SubpixelPointSerial,
    TeamChoiceKindSerialIn, TeamKindSerial, ThirdAbilityKindSerial, WorldShapeKindSerial,
};
use shared::random::Pcg32;
use shared::replay;
use shared::replay::{ReplayLog, ReplayReadError};
use shared::round::RoundPhase;

use crate::helpers::replay_fixture;
```

In `shared/tests/protocol.rs`, insert directly above the line starting `fn get_ability_phase_name(`:

```rust
const MALFORMED_INPUT_SEED_COUNT: u64 = 400;
const MALFORMED_SNAPSHOT_TICK: Tick = Tick(2500);
const MALFORMED_REPLAY_BUNDLE_COUNT: usize = 20;
const RANDOM_FRAME_BYTE_LIMIT: u32 = 64;
const LOADOUT_SERIAL: LoadoutSerial = LoadoutSerial {
    appearance: AppearanceSerial {
        color: OrganismColorKindSerial::Lake,
        skin: SkinKindSerial::Grid,
    },
    first: FirstAbilityKindSerial::Compress,
    second: SecondAbilityKindSerial::Freeze,
    third: ThirdAbilityKindSerial::Toxin,
};

#[derive(Debug)]
struct MalformedInputOutcomeCounts {
    decode_rejected: u32,
    conversion_rejected: u32,
    accepted: u32,
}

impl MalformedInputOutcomeCounts {
    fn new() -> MalformedInputOutcomeCounts {
        MalformedInputOutcomeCounts {
            decode_rejected: 0,
            conversion_rejected: 0,
            accepted: 0,
        }
    }

    fn record<T>(&mut self, decoded: Result<T, AppError>, convert: impl FnOnce(T) -> Result<(), AppError>) {
        let Ok(value): Result<T, AppError> = decoded else {
            self.decode_rejected += 1;
            return;
        };

        let conversion: Result<(), AppError> = convert(value);

        match conversion {
            Ok(()) => self.accepted += 1,
            Err(_) => self.conversion_rejected += 1,
        }
    }

    fn total(&self) -> u32 {
        self.decode_rejected + self.conversion_rejected + self.accepted
    }
}
```

Append to the end of `shared/tests/protocol.rs`, after one blank line:

```rust
/// The state after `tick`, and the bundle of the tick after it.
fn get_state_and_next_bundle(replay_log: &ReplayLog, tick: Tick) -> (GameState, InputBundle) {
    let mut state: GameState = GameState::new(replay_log.header.settings.clone(), replay_log.header.seed);
    let bundle_index: usize = replay_log.input_bundles.iter().position(|bundle| bundle.tick == tick).unwrap();

    for bundle in &replay_log.input_bundles[..=bundle_index] {
        game::step(&mut state, bundle).unwrap();
    }

    (state, replay_log.input_bundles[bundle_index + 1].clone())
}

fn create_server_frames(replay_log: &ReplayLog) -> Vec<Vec<u8>> {
    let (state, next_bundle): (GameState, InputBundle) = get_state_and_next_bundle(replay_log, MALFORMED_SNAPSHOT_TICK);
    let server_messages: Vec<MessageSerialOut> = vec![
        MessageSerialOut::Snapshot(GameSnapshotSerialOut {
            game_id: 1,
            state: GameStateSerialOut::from(&state),
        }),
        MessageSerialOut::InputBundle(InputBundleSerialOut::from(&next_bundle)),
        MessageSerialOut::GameList {
            game_summaries: vec![GameSummarySerialOut {
                game_id: 1,
                title: String::from("Arena"),
                mode: GameModeKindSerial::Skirmish,
                player_count: 3,
                spectator_count: 1,
                player_cap: 8,
                secured: true,
                team_sizes: vec![2, 1],
            }],
            online_client_count: 9,
        },
        MessageSerialOut::RequestRejected {
            request: RequestKindSerialOut::JoinGame,
            reason: RejectionKindSerialOut::TeamUnbalanced {
                requested: TeamKindSerial::Red,
                smaller: TeamKindSerial::Blue,
            },
        },
        MessageSerialOut::CursorCorrected {
            client_tick: 40,
            cursor: SubpixelPointSerial { x: 409_600, y: 3 },
        },
    ];

    server_messages.iter().map(protocol::encode_message_out).collect()
}

fn create_client_frames() -> Vec<Vec<u8>> {
    let joiner_serial_in: JoinerSerialIn = JoinerSerialIn {
        screen_name: String::from("Blob"),
        loadout: LOADOUT_SERIAL,
        team: TeamChoiceKindSerialIn::Team(TeamKindSerial::Green),
    };
    let client_messages: Vec<MessageSerialIn> = vec![
        MessageSerialIn::CreateGame {
            settings: GameSettingsSerialIn {
                title: String::from("Arena"),
                mode: GameModeKindSerial::Skirmish,
                world_shape: WorldShapeKindSerial::Ellipse,
                world_size_pixels: 800,
                player_minimum: None,
                player_cap: 16,
                team_count: Some(4),
                leaderboard_length: 10,
            },
            password: Some(String::from("secret")),
            joiner: joiner_serial_in.clone(),
        },
        MessageSerialIn::JoinGame {
            game_id: 1,
            password: None,
            joiner: joiner_serial_in,
        },
        MessageSerialIn::SpectateGame {
            game_id: 1,
            password: None,
            screen_name: String::from("Watcher"),
        },
        MessageSerialIn::Respawn {
            loadout: LOADOUT_SERIAL,
            team: TeamChoiceKindSerialIn::Auto,
        },
        MessageSerialIn::Input(PlayerInputSerialIn {
            client_tick: 12,
            cursor: SubpixelPointSerial { x: 300_000, y: 200_000 },
            ability_presses: 0b0001,
            aim: Some(AimVectorSerial { x: 50, y: -50 }),
        }),
    ];

    client_messages.iter().map(protocol::encode_message_in).collect()
}

fn create_malformed_frames(frame: &[u8], rng: &mut Pcg32) -> Vec<Vec<u8>> {
    let frame_length: u32 = u32::try_from(frame.len()).unwrap();
    let truncated_length: usize = usize::try_from(rng.below(frame_length)).unwrap();
    let flipped_bit_index: usize = usize::try_from(rng.below(frame_length * 8)).unwrap();
    let mut flipped_frame: Vec<u8> = frame.to_vec();
    flipped_frame[flipped_bit_index / 8] ^= 1 << (flipped_bit_index % 8);
    let mut appended_frame: Vec<u8> = frame.to_vec();
    appended_frame.push(get_random_byte(rng));
    let random_frame_length: u32 = rng.below(RANDOM_FRAME_BYTE_LIMIT);
    let random_frame: Vec<u8> = (0..random_frame_length).map(|_| get_random_byte(rng)).collect();

    vec![
        frame[..truncated_length].to_vec(),
        flipped_frame,
        appended_frame,
        random_frame,
    ]
}

fn get_random_byte(rng: &mut Pcg32) -> u8 {
    u8::try_from(rng.below(256)).unwrap()
}

fn convert_server_message(message_serial_out: MessageSerialOut) -> Result<(), AppError> {
    match message_serial_out {
        MessageSerialOut::GameList { game_summaries, .. } => {
            for game_summary_serial_out in game_summaries {
                GameSummary::try_from(game_summary_serial_out)?;
            }
        }
        MessageSerialOut::Snapshot(snapshot_serial_out) => {
            GameState::try_from(snapshot_serial_out.state)?;
        }
        MessageSerialOut::InputBundle(bundle_serial_out) => {
            InputBundle::try_from(bundle_serial_out)?;
        }
        MessageSerialOut::CursorCorrected { cursor, .. } => {
            SubpixelPoint::try_from(cursor)?;
        }
        MessageSerialOut::JoinAccepted { .. }
        | MessageSerialOut::RequestRejected { .. }
        | MessageSerialOut::StateChecksum { .. }
        | MessageSerialOut::LeftGame
        | MessageSerialOut::GameEnded => {}
    }

    Ok(())
}

fn convert_client_message(message_serial_in: MessageSerialIn) -> Result<(), AppError> {
    match message_serial_in {
        MessageSerialIn::CreateGame {
            settings,
            password,
            joiner,
        } => {
            GameSettings::try_from(settings).map_err(RejectionKind::to_app_error)?;
            protocol_limits::normalize_password(password).map_err(RejectionKind::to_app_error)?;
            Joiner::try_from(joiner).map_err(RejectionKind::to_app_error)?;
        }
        MessageSerialIn::JoinGame { password, joiner, .. } => {
            protocol_limits::normalize_password(password).map_err(RejectionKind::to_app_error)?;
            Joiner::try_from(joiner).map_err(RejectionKind::to_app_error)?;
        }
        MessageSerialIn::SpectateGame {
            password, screen_name, ..
        } => {
            protocol_limits::normalize_password(password).map_err(RejectionKind::to_app_error)?;
            protocol_limits::normalize_screen_name(&screen_name).map_err(RejectionKind::to_app_error)?;
        }
        MessageSerialIn::Input(player_input_serial_in) => {
            PlayerInput::try_from(player_input_serial_in)?;
        }
        MessageSerialIn::SubscribeGameList
        | MessageSerialIn::UnsubscribeGameList
        | MessageSerialIn::Respawn { .. }
        | MessageSerialIn::UpdateAppearance { .. }
        | MessageSerialIn::RequestSnapshot { .. }
        | MessageSerialIn::LeaveGame => {}
    }

    Ok(())
}

#[test]
fn decode_and_try_from_never_panic_on_malformed_frames() {
    let replay_log: ReplayLog = replay_fixture::read_scripted_game_log();
    let server_frames: Vec<Vec<u8>> = create_server_frames(&replay_log);
    let client_frames: Vec<Vec<u8>> = create_client_frames();
    let mut outcome_counts: MalformedInputOutcomeCounts = MalformedInputOutcomeCounts::new();

    for seed in 0..MALFORMED_INPUT_SEED_COUNT {
        let mut rng: Pcg32 = Pcg32::from_seed(seed);

        for server_frame in &server_frames {
            for malformed_frame in create_malformed_frames(server_frame, &mut rng) {
                outcome_counts.record(protocol::decode_message_out(&malformed_frame), convert_server_message);
            }
        }

        for client_frame in &client_frames {
            for malformed_frame in create_malformed_frames(client_frame, &mut rng) {
                outcome_counts.record(protocol::decode_message_in(&malformed_frame), convert_client_message);
            }
        }
    }

    assert_eq!(outcome_counts.total(), 16_000);
    assert!(outcome_counts.decode_rejected > 0);
    assert!(outcome_counts.conversion_rejected > 0);
    assert!(outcome_counts.accepted > 0);
}

#[test]
fn read_replay_never_panics_on_a_malformed_replay() {
    let mut replay_log: ReplayLog = replay_fixture::read_scripted_game_log();
    replay_log.input_bundles.truncate(MALFORMED_REPLAY_BUNDLE_COUNT);
    let replay_bytes: Vec<u8> = replay::write_replay(&replay_log);
    let mut rejected_count: u32 = 0;

    for seed in 0..MALFORMED_INPUT_SEED_COUNT {
        let mut rng: Pcg32 = Pcg32::from_seed(seed);

        for malformed_replay_bytes in create_malformed_frames(&replay_bytes, &mut rng) {
            let read_result: Result<ReplayLog, ReplayReadError> = replay::read_replay(&malformed_replay_bytes);

            if read_result.is_err() {
                rejected_count += 1;
            }
        }
    }

    assert!(rejected_count > 0);
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p shared --test protocol`

Expected: PASS, `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.70s`

It passes at once: decoding and every conversion were written not to panic; the test guards that.

- [ ] **Step 3: Commit**

```sh
git add shared/tests/protocol.rs
git status
git commit -m "malformed input tests"
```

### Task 27: Size budgets

**Files:**
- Modify: `shared/tests/protocol.rs`
- Test: `shared/tests/protocol.rs`

Upper budgets accepted by the owner (testing.md#protocol-tests): an `InputBundle` frame with every player moving, pressing all four keys and aiming is at most 128 B for 8 players and 256 B for 16; a snapshot frame of a full 32-player game, every organism grown for 300 ticks and its spores just launched, is at most 32 KB. Observed while writing this plan: 59 B, 86 B and 12175 B (917 spores, 1167 cells).

- [ ] **Step 1: Write the test**

In `shared/tests/protocol.rs`, replace:

```rust
mod helpers;

use std::collections::BTreeSet;

use shared::ability::{AbilityPhase, OrganismAbilities, ShotPhase, SporePhase};
use shared::error::AppError;
use shared::game;
use shared::game::{GameSettings, GameState, GameSummary, InputBundle, PlayerInput, Tick};
use shared::geometry::SubpixelPoint;
use shared::member::Joiner;
use shared::organism::Organism;
use shared::protocol;
use shared::protocol::protocol_limits;
use shared::protocol::{
    AimVectorSerial, AppearanceSerial, FirstAbilityKindSerial, GameModeKindSerial, GameSettingsSerialIn,
    GameSnapshotSerialOut, GameStateSerialOut, GameSummarySerialOut, InputBundleSerialOut, JoinerSerialIn,
    LoadoutSerial, MessageSerialIn, MessageSerialOut, OrganismColorKindSerial, PlayerInputSerialIn, RejectionKind,
    RejectionKindSerialOut, RequestKindSerialOut, SecondAbilityKindSerial, SkinKindSerial, SubpixelPointSerial,
    TeamChoiceKindSerialIn, TeamKindSerial, ThirdAbilityKindSerial, WorldShapeKindSerial,
};
use shared::random::Pcg32;
use shared::replay;
use shared::replay::{ReplayLog, ReplayReadError};
use shared::round::RoundPhase;

use crate::helpers::replay_fixture;
```

with:

```rust
mod helpers;

use std::collections::BTreeSet;

use shared::ability::{AbilityPhase, AbilityPressSet, AimVector, Loadout, OrganismAbilities, ShotPhase, SporePhase};
use shared::error::AppError;
use shared::game;
use shared::game::{
    GameModeKind, GameSettings, GameState, GameSummary, InputBundle, MemberEvent, PlayerInput, PlayerTickInput, Tick,
};
use shared::geometry::{SubpixelPoint, WorldPoint};
use shared::member::{Joiner, MemberId, MemberRoleKind};
use shared::organism::Organism;
use shared::protocol;
use shared::protocol::protocol_limits;
use shared::protocol::{
    AimVectorSerial, AppearanceSerial, FirstAbilityKindSerial, GameModeKindSerial, GameSettingsSerialIn,
    GameSnapshotSerialOut, GameStateSerialOut, GameSummarySerialOut, InputBundleSerialOut, JoinerSerialIn,
    LoadoutSerial, MessageSerialIn, MessageSerialOut, OrganismColorKindSerial, PlayerInputSerialIn, RejectionKind,
    RejectionKindSerialOut, RequestKindSerialOut, SecondAbilityKindSerial, SkinKindSerial, SubpixelPointSerial,
    TeamChoiceKindSerialIn, TeamKindSerial, ThirdAbilityKindSerial, WorldShapeKindSerial,
};
use shared::random::Pcg32;
use shared::replay;
use shared::replay::{ReplayLog, ReplayReadError};
use shared::round::RoundPhase;
use shared::world::WorldShapeKind;

use crate::helpers::replay_fixture;
```

In `shared/tests/protocol.rs`, replace:

```rust
    third: ThirdAbilityKindSerial::Toxin,
};
```

with:

```rust
    third: ThirdAbilityKindSerial::Toxin,
};
const SIZE_BUDGET_TICK: Tick = Tick(1_000_000);
const GROWTH_TICK_COUNT: u32 = 300;
const SNAPSHOT_SIZE_BUDGET_BYTES: usize = 32 * 1024;
```

Append to the end of `shared/tests/protocol.rs`, after one blank line:

```rust
fn create_pressing_bundle(player_count: u32) -> InputBundle {
    let every_press: AbilityPressSet = AbilityPressSet::FIRST
        .with(AbilityPressSet::SECOND)
        .with(AbilityPressSet::THIRD)
        .with(AbilityPressSet::FOURTH);
    let player_inputs: Vec<PlayerTickInput> = (0..player_count)
        .map(|player_index| {
            let offset: i32 = i32::try_from(player_index).unwrap();

            PlayerTickInput {
                member_id: MemberId(player_index),
                cursor: WorldPoint {
                    x: 99_000 - 3 * offset,
                    y: 98_000 + 3 * offset,
                },
                ability_presses: every_press,
                aim: Some(AimVector {
                    x: -1000 + i16::try_from(offset).unwrap(),
                    y: 1000,
                }),
            }
        })
        .collect();

    InputBundle {
        tick: SIZE_BUDGET_TICK,
        member_events: Vec::new(),
        player_inputs,
    }
}

fn get_bundle_frame_length(bundle: &InputBundle) -> usize {
    protocol::encode_message_out(&MessageSerialOut::InputBundle(InputBundleSerialOut::from(bundle))).len()
}

fn create_empty_bundle(tick: Tick) -> InputBundle {
    InputBundle {
        tick,
        member_events: Vec::new(),
        player_inputs: Vec::new(),
    }
}

fn create_full_game_settings() -> GameSettings {
    GameSettings {
        title: String::from("Full game"),
        mode: GameModeKind::FreeForAll,
        world_shape: WorldShapeKind::Rectangle,
        world_width_pixels: 3000,
        world_height_pixels: 3000,
        player_minimum: None,
        player_cap: protocol_limits::PLAYER_CAP_HIGHEST,
        team_count: None,
        leaderboard_length: 10,
    }
}

fn create_joined_players_bundle(player_count: u32) -> InputBundle {
    let loadout: Loadout = Loadout::from(LOADOUT_SERIAL);
    let member_events: Vec<MemberEvent> = (0..player_count)
        .flat_map(|player_index| {
            [
                MemberEvent::Joined {
                    member_id: MemberId(player_index),
                    screen_name: format!("player {player_index}"),
                    role: MemberRoleKind::Participant,
                    loadout: Some(loadout),
                    team: None,
                },
                MemberEvent::SpawnRequested {
                    member_id: MemberId(player_index),
                    loadout,
                    team: None,
                },
            ]
        })
        .collect();

    InputBundle {
        tick: Tick(1),
        member_events,
        player_inputs: Vec::new(),
    }
}

fn create_spore_press_bundle(state: &GameState) -> InputBundle {
    let player_inputs: Vec<PlayerTickInput> = state
        .members
        .values()
        .filter_map(|member| member.organism.as_ref().map(|organism| (member.member_id, organism.cursor)))
        .map(|(member_id, cursor)| PlayerTickInput {
            member_id,
            cursor,
            ability_presses: AbilityPressSet::FOURTH,
            aim: None,
        })
        .collect();

    InputBundle {
        tick: state.tick.next(),
        member_events: Vec::new(),
        player_inputs,
    }
}

fn create_full_game_with_spores_in_flight() -> GameState {
    let mut state: GameState = GameState::new(create_full_game_settings(), 7);
    let player_cap: u32 = u32::from(protocol_limits::PLAYER_CAP_HIGHEST);

    game::step(&mut state, &create_joined_players_bundle(player_cap)).unwrap();

    for tick in 2..=GROWTH_TICK_COUNT {
        game::step(&mut state, &create_empty_bundle(Tick(tick))).unwrap();
    }

    let spore_press_bundle: InputBundle = create_spore_press_bundle(&state);
    game::step(&mut state, &spore_press_bundle).unwrap();

    state
}

fn is_launching_spores(state: &GameState) -> bool {
    state.members.values().all(|member| {
        member.organism.as_ref().is_some_and(
            |organism| matches!(&organism.abilities.spore, SporePhase::Flying { spores, .. } if !spores.is_empty()),
        )
    })
}

#[test]
fn input_bundle_frame_fits_the_size_budgets() {
    assert!(get_bundle_frame_length(&create_pressing_bundle(8)) <= 128);
    assert!(get_bundle_frame_length(&create_pressing_bundle(16)) <= 256);
}

#[test]
fn snapshot_frame_at_the_player_cap_with_spores_in_flight_fits_the_size_budget() {
    let state: GameState = create_full_game_with_spores_in_flight();
    let snapshot_frame: Vec<u8> = protocol::encode_message_out(&MessageSerialOut::Snapshot(GameSnapshotSerialOut {
        game_id: 1,
        state: GameStateSerialOut::from(&state),
    }));

    assert_eq!(state.alive_organism_count(), 32);
    assert!(is_launching_spores(&state));
    assert!(snapshot_frame.len() <= SNAPSHOT_SIZE_BUDGET_BYTES);
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p shared --test protocol`

Expected: PASS, `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.81s`

It passes at once: the budgets bound the wire types of Tasks 13 and 17.

- [ ] **Step 3: Commit**

```sh
git add shared/tests/protocol.rs
git status
git commit -m "size budget tests"
```

### Task 28: Phase verification

**Files:** none; verification only.

The roadmap's done condition for this phase: round-trip, model-to-wire-to-model, malformed-input and size-budget tests and the replay fixture test pass, with the rest of the workspace still building for every target.

- [ ] **Step 1: Check formatting**

Run: `cargo fmt --all -- --check`

Expected: no output.

- [ ] **Step 2: Run every test of the workspace**

Run: `cargo test --workspace 2>&1 | grep -E '^test result|FAILED'`

Expected output:

```text
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 325 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.30s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.80s
test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 6.60s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Every line reads `ok`; in order, the binaries are `protocol_dump`, `server` (library and binary), the `shared` library, `determinism`, `forbidden_operations`, `growth_statistics`, `protocol`, `replay`, the `web` targets and the doc tests.

- [ ] **Step 3: Check the wasm32 builds**

Run: `./scripts/test/check-wasm.sh`

Expected: exit status 0, with no `warning` or `error` line.

- [ ] **Step 4: Confirm the working tree is clean**

Run: `git status --short`

Expected: no output.

- [ ] **Step 5: List the phase's commits**

Run: `git log --oneline --reverse $(git log --grep '^>>> branch: protocol' --format=%h)..HEAD | cut -d' ' -f2-`

Expected, oldest first (review-fix commits, if any, sit after their task):

```text
phase 4 plan
app error
protocol limits
team names
rejection kinds
rejection wire types
text limits
game settings validation
close reasons and protocol version
geometry wire conversions
organism wire conversions
member wire conversions
game state wire conversions
input bundle wire types
joiner and game settings request payloads
player input and inbound messages
game summary wire type
message codec
replay header
replay record stream
run replay
protocol dump frame
protocol dump replay
scripted game replay fixture
snapshot round trip test
model to wire to model over the scripted game
malformed input tests
size budget tests
```

## Roadmap coverage

- Wire types with `From`/`TryFrom` conversions: Task 5 (rejection mirrors), Tasks 9 to 12 (the state tree), Task 13 (bundle), Tasks 14 and 15 (request payloads), Task 16 (game summary), Task 18 (replay header).
- `MessageSerialIn`/`MessageSerialOut`: Tasks 15 and 17.
- Encode and decode: Task 17 (messages and game state), Task 18 (replay records).
- `validate()`: Task 7 (`GameSettings::validate`), Task 6 (screen name, title and password rules), Tasks 14 and 15 (the request payload conversions using them).
- Limits: Tasks 2 and 6.
- Rejection and close-reason kinds: Tasks 4, 5 and 8.
- Replay format (`replay/`): Tasks 18 to 20, fixture in Task 23.
- Snapshot: Task 17 (`GameSnapshotSerialOut`), Task 24 (round trip).
- `tools/protocol_dump`: Tasks 21 and 22.
- Done condition: round trips in Tasks 17 and 18; model to wire to model in Tasks 9 to 13 and 25; malformed input in Task 26; size budgets in Task 27; replay fixture tests in Task 23; all checked together in Task 28.
- testing.md#protocol-tests, bullet by bullet: message and wire type round trips (Tasks 17, 18); model to wire to model (Tasks 9 to 13, 16, 18, 25); `SerialIn` acceptance and rejection (Tasks 14, 15); byte-equal encodings of equal cell sets (Task 17); `TryFrom` rejections (Tasks 10, 12, 13, 15); malformed input (Task 26); semantic validation of names, titles, passwords, settings ranges, cross-field rules and `aim` (Tasks 6, 7, 14, 15); size tests (Task 27). The non-increasing `client_tick` rule is phase 5's.
- Left to later phases: `AppErrorStatic`, the protocol violation causes, the server's use of the limits and summaries (phase 5); the client session's use of the codec (phase 6); the form field of each rejection and the create form's defaults (phase 8); `scripts/test/test-replay-wasm.sh` (phase 9).

## PR description

**Problem:** the simulation could only encode its state for the checksum; nothing could decode a frame, validate a request, or record and replay a game.

**Solution:** the full protocol layer in `shared/src/protocol/` (every wire type with its `From`/`TryFrom` conversions, `MessageSerialIn`/`MessageSerialOut` and their codec, `PROTOCOL_VERSION`, limits and validation, rejection and close-reason kinds), the `replay` module with a committed `scripted_game.replay` fixture whose checksums are unchanged from phase 3, and `tools/protocol_dump` for frames and replays. Repository: `bacter-rs` only.
