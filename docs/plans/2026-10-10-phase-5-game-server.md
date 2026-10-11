# Phase 5: game server implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** the `server` crate: an actix-web game server which accepts the binary WebSocket at `/ws`, runs one deterministic tick task per game, admits joiners, spectators and respawns, clamps cursors, builds and broadcasts input bundles, handles slow consumers, broadcasts the lobby list, hashes passwords, rate-limits connections, captures replays, serves the static site and `/health`, and binds plain HTTP or rustls; proven by unit tests of every module and the integration tests of `docs/architecture/testing.md` against in-process servers.

**Architecture:** `docs/architecture/server.md`. Two tasks per socket on the accepting actix worker: a read task (`socket/connection.rs`) which owns the connection state and a writer task (`socket/socket_writer.rs`) which alone owns the `actix_ws::Session`. Each game is a `GameTask` on the static `GAME_RUNTIME`, fed by bounded request and input channels and feeding one bounded membership queue per member; a supervisor task unregisters it however it ends. Every registry is a static reached only through the `Copy` struct `ServerRegistries`, so tests run parallel in-process servers over `ServerRegistries::leaked()`.

**Tech Stack:** Rust 1.95 (edition 2024), actix-web 4.13 (feature `rustls-0_23`), actix-ws 0.4, actix-files 0.7, rustls 0.23 (`PemObject`), tokio 1.52 (`full`), dashmap 6.1, bcrypt 0.17, getrandom 0.4, bytes 1.11, futures-util 0.3, env_logger 0.11, dotenvy 0.15, minimer 2.2, tokio-tungstenite 0.26 (dev); no new dependencies.

---

## Ground rules for the executor

- Repository `/Users/singularity/bacter-rs`, branch `game-server`. Run `git -C /Users/singularity/bacter-rs branch --show-current` first; stop if it does not print `game-server`.
- Every command runs from the repository root `/Users/singularity/bacter-rs`.
- Never switch branches, push, rebase or amend. Stage by explicit path, run `git status` before each commit, one-line commit messages without attribution.
- Never run `cargo clippy`.
- The code below is `rustfmt`-formatted with the repository's `rustfmt.toml`; transcribe it exactly, so `cargo fmt --all -- --check` stays silent.
- Edits take these forms:
  - "Create `path`" with the whole new file;
  - "Replace the whole contents of `path`";
  - "In `path`, replace" a text which occurs exactly once in the file, "with" its replacement;
  - "replace everything above `#[cfg(test)]`" (the file's one test module) with the given code;
  - "append to the end of `path`, after one blank line";
  - "append inside `mod tests`, directly above its closing brace", which is the last line of the file, after one blank line separating it from the item before.
- A new file's failing test is the file holding only its `#[cfg(test)] mod tests` block (together with its `pub mod` line); the implementation step then replaces everything above `#[cfg(test)]`, which is nothing yet, with the code, followed by one blank line.
- `mod.rs` files and `server/src/lib.rs` are given whole each time they change.
- Builds stay warning-free, tests included; a warning is a defect to fix before committing.
- Expected outputs were observed while writing this plan, by executing it end to end in a scratch copy of this branch; test counts and filtered counts assume the tasks are done in order with the code exactly as given, and only the `finished in` timings vary. A compile failure is quoted by its first `error` line; later lines differ only in the names they report.
- From Task 24 on, the integration tests start in-process servers on `127.0.0.1:0`; the sandbox must allow binding loopback ports. A bind failing with `Operation not permitted (os error 1)` (`the test server binds` panics) means the command ran sandboxed: run it from a script in `/tmp`, in the foreground.
- Task 26 tests behaviour finished in earlier tasks, so its first run passes; it says so where it runs.

## Decisions taken where the design is silent or contradictory

- `ServerConfig` carries `tick_period` beside the design's `empty_game_grace_period`, both defaulting to the design's values (70 ms, 10 s) and set shorter only by tests: the respawn test runs at 10 ms per tick and the slow-consumer test at 1 ms. The stall test needs a few hundred kilobytes of frames to fill the loopback socket buffers, which game traffic at 70 ms per tick takes hours to produce.
- The integration test of a member which stops reading takes about 45 s, so it is `#[ignore]` and runs through the new `scripts/test/test-slow-consumer.sh` (testing conventions: a long run is ignored and scripted), listed in `docs/architecture/dev-workflow.md`. The stalled client keeps sending `RequestSnapshot` and never reads, so the inbound timeout cannot remove it first; the server removes it when a socket write times out.
- Close causes: `ConnectionFlow::Close`, `WriterCommand::Close` and `ConnectionControl::Close` carry `CloseCauseKind { Reason(CloseReasonKind), ProtocolViolation(ProtocolViolationKind), ClientClosed }` rather than a bare `CloseReasonKind`, so a protocol violation's close reason names its cause (`decode`, `conversion`, `text frame`, `oversize`, `validation`) and a client's own close is answered with 1000. `ConnectionFlow` also has `Finish`, for an end without a close frame (stream ended, writer gone). `ConnectionControl` is `Open | Close(CloseCauseKind)`.
- `WriterCommand` also has `Ping` and `Pong(Bytes)`: the read task's pings and pong answers go through the writer, which alone owns the session.
- `/ws` refuses a handshake without an `Origin` header like one with a disallowed origin: HTTP 403, no upgrade.
- Per-IP limits (connections, password failures) key on the TCP peer address. The design is silent on forwarded addresses; behind Cloudflare every client shares the proxy's addresses, so the 16-connections-per-IP limit would bind there. No forwarded header is trusted; this is reported as an open point.
- `SubscribeGameList` takes a request token (it has a `RequestKind`, so it can be answered `RateLimited`, and it builds a list); `UnsubscribeGameList` takes none. `Input` and `RequestSnapshot` outside a game are dropped; a `PlayerInputSerialIn` which fails its conversion closes the connection in any phase.
- `client_tick` greater than the server tick is judged in the game task, the only holder of the tick, and closes the connection through its control watch with `ProtocolViolation` (`validation`).
- Teams: `Auto` chooses uniformly among the smallest teams with a `team_assignment_rng: &mut Pcg32` which `admit_join` and `admit_respawn` take (simulation.md, teams: the server's own RNG, never the game's); the game task holds it, seeded from `getrandom`, and passes it at the Task 19 call sites; a requested team outside the game is `TeamUnbalanced { requested, smaller: <first smallest team> }`; a respawning member's own team is excluded from the sizes it is compared against.
- Respawn: of a member whose `Joined` is still pending, `NotDead`; of a member id below `next_member_id` but not in the state, or from a connection the game no longer serves, `NotInGame`. A respawning Spectator becomes a Participant (the simulation's `spawn_member`).
- `UpdateAppearance` has no reply; only the read task answers it (`NotInGame`, `RateLimited`, `ServerBusy`, `GameNotFound`).
- Cursor clamp: the budget grows by `3046 + 128` subpixels per tick up to 15 ticks of that (47610 subpixels) and starts empty at each spawn; distances are Euclidean; an input applies once, and a clamped input's remainder is dropped (the client learns of it from `CursorCorrected`). The bundle cursor rounds subpixels to whole pixels, halves up.
- Snapshots: a requested snapshot waits until 29 ticks after the last snapshot sent to that member (the join snapshot included); the snapshot which resumes a drained slow consumer has no minimum interval. Bundles and checksums skip a recipient awaiting either.
- Lobby: the lobby task polls every 250 ms and sends at most one list per second, only after the revision changed; the revision counts changes of the whole list (summaries plus the online client count, which is the accepted connection count). `SubscribeGameList` sends the current list through the same lobby queue.
- `ServerRegistries` also holds `next_connection_id` (static `NEXT_CONNECTION_ID` in `socket/connection_model.rs`), so no code outside the registries names a registry static. Game and connection ids start at 1.
- Creation runs in the read task (`request_dispatch::create_game`); `game_supervisor::launch_game` registers the handle and spawns the game task and its supervisor on `GAME_RUNTIME`. `GameTaskConfig` is the part of `ServerConfig` a game needs.
- `GameSummary::from_game_state` lives in `shared` beside `GameSummary`; team sizes count Participants per team, alive or not.
- `site_api::configurer(service_config, site_root)` takes the site root as a second parameter, since actix-files needs the directory when the routes are built; cache headers come from a `middleware::from_fn` on a scope around the files service. `/health` answers `ok` with or without a site.
- Shutdown: `server::run` handles SIGINT and SIGTERM itself (`disable_signals`), publishes `ServerRunState::ShuttingDown` on a watch every read task observes (close 1001, `server shutting down`), waits 500 ms for the close frames, then stops actix gracefully. The new module `server/src/shutdown/` holds `ServerRunState` and the signal wait; `CreatedServer` hands tests the watch sender.
- Replay capture: `File::create` (game ids restart with the process, so a capture of an earlier run with the same id is overwritten); the directory is created when absent; the channel holds 1024 records.
- Passwords: semaphore exhaustion and hashing failures answer `ServerBusy`; the per-IP `RateLimited` check runs before hashing; the fifth wrong password on one connection closes it with 4003 instead of answering `PasswordIncorrect`.
- A `StepError` ends the game (its members get `GameEnded`); it cannot happen with server-built bundles.
- The connection's phase is derived from its joined game, so the two cannot disagree. `AppErrorStatic` arrives in `shared` with its first consumer, the password check on `web::block`.
- TLS is tested by building a `rustls::ServerConfig` from a committed self-signed `localhost` certificate (`server/tests/fixtures/`), generated by an `openssl` command in Task 4; no end-to-end TLS test, since tokio-tungstenite has no TLS feature here.
- Shared integration helpers start with `#![allow(dead_code)]`, as eafora's do, because each test binary uses only part of them.

## File structure

- Modify `shared/src/error/error.rs`: `AppErrorStatic` and the conversions both ways.
- Modify `shared/src/game/game_summary.rs`: `GameSummary::from_game_state`.
- Modify `shared/src/ability/ability_model.rs`: `AbilityPressSet::has_shot_slot_press`.
- Modify `shared/src/protocol/message_in.rs`: `TryFrom<PlayerInputSerialIn>` uses it.
- Modify `server/src/lib.rs`: module declarations, `CreatedServer`, `create_server`, `run`.
- Modify `server/src/main.rs`: `.env`, logger, configuration, `server::run`.
- Create `server/src/config/mod.rs`, `server/src/config/config.rs`: `ServerConfig`, `TlsFiles`, the environment variables.
- Create `server/src/config/tls.rs`: the rustls `ServerConfig` from the PEM files.
- Create `server/tests/fixtures/localhost.certificate.pem`, `server/tests/fixtures/localhost.key.pem`: self-signed test certificate and key (generated).
- Create `server/src/socket/mod.rs`: declares the socket modules.
- Create `server/src/socket/rate_limit.rs`: `TokenBucket`, the input and request rates, abuse detection.
- Create `server/src/socket/connection_model.rs`: `ConnectionId`, `ConnectionPhase`, `ConnectionControl`, `CloseCauseKind`, `ProtocolViolationKind`, `Membership`, `NEXT_CONNECTION_ID`, frame encoding.
- Create `server/src/socket/connection_limit.rs`: `CONNECTION_COUNT`, `CONNECTIONS_PER_IP`, reservation and release.
- Create `server/src/socket/socket_writer.rs`: `WriterCommand`, the writer task, the write timeout, closing a session.
- Create `server/src/socket/connection.rs`: `Connection`, `JoinedGame`, `ConnectionFlow`, the read task.
- Create `server/src/socket/request_dispatch.rs`: routing by phase, request payload conversions, creation, joins, passwords, rate limits.
- Create `server/src/socket/socket_api.rs`: `SocketContext`, `configurer` for `GET /ws`, origin, version and connection-limit checks.
- Create `server/src/game/mod.rs`: declares the game modules.
- Create `server/src/game/cursor_clamp.rs`: `CursorClamp`.
- Create `server/src/game/game_runtime.rs`: `GAME_RUNTIME`, the CPU count.
- Create `server/src/game/game_handle.rs`: `GameHandle`, `GameRequest`, `GameInput`, `ConnectionEndpoint`, channel capacities, `REQUEST_SEND_TIMEOUT`.
- Create `server/src/game/game_registry.rs`: `GAMES`, `GAME_TITLES`, `NEXT_GAME_ID`, `GAME_COUNT`, `GameReservation`, reserve, insert, look up, remove.
- Create `server/src/game/test_fixture.rs`: unit-test fixtures (test builds only).
- Create `server/src/game/pending_admissions.rs`: `PendingAdmissions`.
- Create `server/src/game/membership.rs`: join, spectate and respawn admission, team resolution.
- Create `server/src/game/bundle_builder.rs`: `BundleBuilder`, input acceptance, `CursorCorrection`.
- Create `server/src/game/recipients.rs`: `Recipients`, fan-out, snapshot scheduling, slow consumers, liveness.
- Create `server/src/game/replay_capture.rs`: `ReplayCapture` and its writer task.
- Create `server/src/game/game_task.rs`: `GameTask`, `TickLag`, the tick loop.
- Create `server/src/game/game_supervisor.rs`: `GameTaskConfig`, `launch_game`, the supervisor.
- Create `server/src/password/mod.rs`, `server/src/password/password.rs`: bcrypt on `web::block`, `PASSWORD_CHECK_SEMAPHORE`, `PASSWORD_FAILURE_LIMIT`.
- Create `server/src/password/password_failure_limit.rs`: `PASSWORD_FAILURES_PER_IP`, the 10-minute failure window.
- Create `server/src/registries/mod.rs`, `server/src/registries/registries.rs`: `ServerRegistries`.
- Create `server/src/lobby/mod.rs`, `server/src/lobby/game_list.rs`: `GameList` from the registered games.
- Create `server/src/lobby/lobby_broadcast.rs`: `LOBBY_SUBSCRIBERS`, subscribe, `LobbyRevision`, the lobby task.
- Create `server/src/site/mod.rs`, `server/src/site/site_api.rs`: static files with cache headers, `GET /health`.
- Create `server/src/shutdown/mod.rs`, `server/src/shutdown/shutdown.rs`: `ServerRunState`, the termination signal wait.
- Create `server/tests/helpers/mod.rs`, `server/tests/helpers/test_server.rs`: in-process test server, `TestClient`, `TestPlayer`, message builders.
- Create `server/tests/protocol_errors.rs`: transport, close and rejection integration tests.
- Create `server/tests/game_lifecycle.rs`: game lifecycle integration tests, replay capture, clamp, the ignored slow-consumer test.
- Create `scripts/test/test-slow-consumer.sh`: runs the ignored slow-consumer test.
- Modify `docs/architecture/dev-workflow.md`: lists the new script.

---

### Task 1: Error type for work on another thread

**Files:**
- Modify: `shared/src/error/error.rs`
- Test: inline `mod tests` in `shared/src/error/error.rs`

Password hashing runs on `web::block`, whose closure must return a `Send` error (server.md, passwords). `AppErrorStatic` is minimer's `Send` error newtype, with both conversions, as in eafora's `shared/src/error.rs`.

- [ ] **Step 1: Write the failing test**

In `shared/src/error/error.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn app_error_static_from_keeps_the_message_and_the_source_text() {
        let source_error: std::io::Error = std::io::Error::other("disk full");
        let error: AppError = AppError::from_error("cannot write the replay", Box::new(source_error));
        let static_error: AppErrorStatic = AppErrorStatic::from(error);

        assert_eq!(static_error.message, "Error: cannot write the replay -> disk full");
    }

    #[test]
    fn app_error_from_keeps_the_message_of_a_static_error() {
        let static_error: AppErrorStatic = AppErrorStatic::new("hashing failed");
        let error: AppError = AppError::from(static_error);

        assert_eq!(error.message, "Error: hashing failed");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib error::error`

Expected: compile failure, first line ``error[E0425]: cannot find type `AppErrorStatic` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/error/error.rs`, replace everything above `#[cfg(test)]` with:

```rust
minimer::define_app_error!(pub AppError);

// For work moved to another thread, which needs a `Send` result; keeps the message but no source error.
minimer::define_app_error_static!(pub AppErrorStatic);

impl From<AppError> for AppErrorStatic {
    fn from(error: AppError) -> AppErrorStatic {
        AppErrorStatic(minimer::AppErrorStatic::from(error.0))
    }
}

impl From<AppErrorStatic> for AppError {
    fn from(error: AppErrorStatic) -> AppError {
        AppError(minimer::AppError::from(error.0))
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib error::error`

Expected: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 328 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/error/error.rs
git commit -m "app error static"
```

### Task 2: Game summary from a game state

**Files:**
- Modify: `shared/src/game/game_summary.rs`
- Test: inline `mod tests` in `shared/src/game/game_summary.rs`

The game task publishes a `GameSummary` after every tick (server.md, game task, step 5); the lobby lists it. `player_count` counts alive organisms, `spectator_count` the members without one, `team_sizes` the Participants per team in skirmish (protocol.md, server to client).

- [ ] **Step 1: Write the failing test**

Append to the end of `shared/src/game/game_summary.rs`, after one blank line:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::geometry::WorldPoint;
    use crate::member::{Member, MemberId, MemberRoleKind};
    use crate::world::WorldShapeKind;

    #[test]
    fn from_game_state_counts_alive_organisms_and_members_without_one() {
        let mut state: GameState =
            test_fixture::create_state_with_organisms(GameModeKind::FreeForAll, 800, &[WorldPoint { x: 100, y: 100 }]);
        state.members.insert(MemberId(1), test_fixture::create_participant(MemberId(1)));
        let mut spectator: Member = test_fixture::create_participant(MemberId(2));
        spectator.role = MemberRoleKind::Spectator;
        spectator.loadout = None;
        state.members.insert(MemberId(2), spectator);

        assert_eq!(
            GameSummary::from_game_state(GameId(4), &state, true),
            GameSummary {
                game_id: GameId(4),
                title: String::from("Fixture game"),
                mode: GameModeKind::FreeForAll,
                player_count: 1,
                spectator_count: 2,
                player_cap: 8,
                secured: true,
                team_sizes: Vec::new(),
            },
        );
    }

    #[test]
    fn from_game_state_lists_participants_per_team_in_skirmish() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, WorldShapeKind::Rectangle, 800);

        for (member_id, team) in [(0, TeamKind::Blue), (1, TeamKind::Blue), (2, TeamKind::Red)] {
            let mut member: Member = test_fixture::create_participant(MemberId(member_id));
            member.team = Some(team);
            state.members.insert(MemberId(member_id), member);
        }

        assert_eq!(
            GameSummary::from_game_state(GameId(0), &state, false).team_sizes,
            vec![1, 2]
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib game::game_summary`

Expected: compile failure, first line ``error[E0425]: cannot find type `GameState` in this scope``.

- [ ] **Step 3: Implement**

In `shared/src/game/game_summary.rs`, replace everything above `#[cfg(test)]` with:

```rust
use crate::game::{GameModeKind, GameState};
use crate::member;
use crate::member::{TeamKind, TeamSizes};

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

impl GameSummary {
    pub fn from_game_state(game_id: GameId, state: &GameState, secured: bool) -> GameSummary {
        let member_count: usize = state.members.len();
        let alive_organism_count: u32 = state.alive_organism_count();
        let spectator_count: usize = member_count.saturating_sub(usize::try_from(alive_organism_count).unwrap_or(0));

        GameSummary {
            game_id,
            title: state.settings.title.clone(),
            mode: state.settings.mode,
            player_count: u8::try_from(alive_organism_count).unwrap_or(u8::MAX),
            spectator_count: u8::try_from(spectator_count).unwrap_or(u8::MAX),
            player_cap: state.settings.player_cap,
            secured,
            team_sizes: get_team_sizes(state),
        }
    }
}

/// Empty outside skirmish.
fn get_team_sizes(state: &GameState) -> Vec<u8> {
    let Some(team_count): Option<u8> = state.settings.team_count else {
        return Vec::new();
    };

    let team_sizes: TeamSizes = TeamSizes::from_members(&state.members, team_count, None);

    member::get_game_teams(team_count)
        .into_iter()
        .map(|team: TeamKind| team_sizes.get_size(team).unwrap_or(0))
        .map(|team_size: u32| u8::try_from(team_size).unwrap_or(u8::MAX))
        .collect()
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p shared --lib game::game_summary`

Expected: `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 331 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/game/game_summary.rs
git commit -m "game summary from game state"
```

### Task 3: Server configuration

**Files:**
- Create: `server/src/config/mod.rs`
- Create: `server/src/config/config.rs`
- Modify: `server/src/lib.rs`
- Test: inline `mod tests` in `server/src/config/config.rs`

`ServerConfig` reads the variables of server.md (process); `from_variables` takes a lookup function so the tests need no process environment. `tick_period` is the test knob of the decisions above.

- [ ] **Step 1: Write the failing test**

Create `server/src/config/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::net::Ipv4Addr;

    fn from_pairs(pairs: &[(&str, &str)]) -> Result<ServerConfig, AppError> {
        let variables: BTreeMap<String, String> =
            pairs.iter().map(|(name, value)| (String::from(*name), String::from(*value))).collect();

        ServerConfig::from_variables(|name: &str| variables.get(name).cloned())
    }

    #[test]
    fn from_variables_uses_the_defaults_when_nothing_is_set() {
        let config: ServerConfig = from_pairs(&[]).unwrap();

        assert_eq!(
            config,
            ServerConfig {
                bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: 3100,
                site_root: None,
                allowed_origins: vec![
                    String::from("http://127.0.0.1:3011"),
                    String::from("http://localhost:3011"),
                ],
                replay_capture_directory: None,
                tls_files: None,
                empty_game_grace_period: Duration::from_secs(10),
                tick_period: Duration::from_millis(70),
            },
        );
    }

    #[test]
    fn from_variables_reads_every_variable() {
        let config: ServerConfig = from_pairs(&[
            (BIND_ADDRESS_VARIABLE, "0.0.0.0"),
            (PORT_VARIABLE, "443"),
            (SITE_ROOT_VARIABLE, "target/site"),
            (
                ALLOWED_ORIGINS_VARIABLE,
                " https://bacter.example/ , https://www.bacter.example",
            ),
            (REPLAY_CAPTURE_DIRECTORY_VARIABLE, "/tmp/bacter/replays"),
            (TLS_CERTIFICATE_PATH_VARIABLE, "/etc/bacter/chain.pem"),
            (TLS_PRIVATE_KEY_PATH_VARIABLE, "/etc/bacter/key.pem"),
        ])
        .unwrap();

        assert_eq!(config.bind_address, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        assert_eq!(config.port, 443);
        assert_eq!(config.site_root, Some(PathBuf::from("target/site")));
        assert_eq!(
            config.allowed_origins,
            vec![
                String::from("https://bacter.example"),
                String::from("https://www.bacter.example"),
            ],
        );
        assert_eq!(
            config.replay_capture_directory,
            Some(PathBuf::from("/tmp/bacter/replays"))
        );
        assert_eq!(
            config.tls_files,
            Some(TlsFiles {
                certificate_chain_path: PathBuf::from("/etc/bacter/chain.pem"),
                private_key_path: PathBuf::from("/etc/bacter/key.pem"),
            }),
        );
    }

    #[test]
    fn from_variables_treats_an_empty_variable_as_unset() {
        let config: ServerConfig = from_pairs(&[(SITE_ROOT_VARIABLE, ""), (PORT_VARIABLE, " ")]).unwrap();

        assert_eq!(config.site_root, None);
        assert_eq!(config.port, 3100);
    }

    #[test]
    fn from_variables_rejects_only_one_tls_variable() {
        assert!(from_pairs(&[(TLS_CERTIFICATE_PATH_VARIABLE, "/etc/bacter/chain.pem")]).is_err());
        assert!(from_pairs(&[(TLS_PRIVATE_KEY_PATH_VARIABLE, "/etc/bacter/key.pem")]).is_err());
    }

    #[test]
    fn from_variables_rejects_an_invalid_port_or_address() {
        assert!(from_pairs(&[(PORT_VARIABLE, "70000")]).is_err());
        assert!(from_pairs(&[(BIND_ADDRESS_VARIABLE, "localhost")]).is_err());
    }

    #[test]
    fn is_origin_allowed_matches_exactly() {
        let config: ServerConfig = from_pairs(&[]).unwrap();

        assert!(is_origin_allowed(&config.allowed_origins, "http://localhost:3011"));
        assert!(!is_origin_allowed(&config.allowed_origins, "http://localhost:3012"));
        assert!(!is_origin_allowed(&config.allowed_origins, "http://evil.example"));
    }
}
```

Create `server/src/config/mod.rs`:

```rust
pub mod config;

pub use config::*;
```

Replace the whole contents of `server/src/lib.rs` with:

```rust
//! The game server.

pub mod config;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib config::config`

Expected: compile failure, first line ``error[E0425]: cannot find type `ServerConfig` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/config/config.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use shared::error::AppError;
use shared::game::tick;

pub const EMPTY_GAME_GRACE_PERIOD: Duration = Duration::from_secs(10);
pub const BIND_ADDRESS_VARIABLE: &str = "BACTER_BIND_ADDRESS";
pub const PORT_VARIABLE: &str = "BACTER_PORT";
pub const SITE_ROOT_VARIABLE: &str = "BACTER_SITE_ROOT";
pub const ALLOWED_ORIGINS_VARIABLE: &str = "BACTER_ALLOWED_ORIGINS";
pub const REPLAY_CAPTURE_DIRECTORY_VARIABLE: &str = "BACTER_REPLAY_CAPTURE_DIRECTORY";
pub const TLS_CERTIFICATE_PATH_VARIABLE: &str = "BACTER_TLS_CERTIFICATE_PATH";
pub const TLS_PRIVATE_KEY_PATH_VARIABLE: &str = "BACTER_TLS_PRIVATE_KEY_PATH";
const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 3100;
/// The web crate's `site-addr`.
const DEFAULT_ALLOWED_ORIGINS: &str = "http://127.0.0.1:3011,http://localhost:3011";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerConfig {
    pub bind_address: IpAddr,
    /// Zero binds any free port.
    pub port: u16,
    /// None serves no static files.
    pub site_root: Option<PathBuf>,
    /// Page origins allowed to open `/ws`.
    pub allowed_origins: Vec<String>,
    pub replay_capture_directory: Option<PathBuf>,
    /// None binds plain HTTP.
    pub tls_files: Option<TlsFiles>,
    pub empty_game_grace_period: Duration,
    pub tick_period: Duration,
}

impl ServerConfig {
    pub fn from_environment() -> Result<ServerConfig, AppError> {
        ServerConfig::from_variables(|name: &str| std::env::var(name).ok())
    }

    /// An empty variable counts as unset.
    pub fn from_variables(get_variable: impl Fn(&str) -> Option<String>) -> Result<ServerConfig, AppError> {
        let get_set_variable = |name: &str| get_variable(name).filter(|value: &String| !value.trim().is_empty());

        let bind_address_text: String =
            get_set_variable(BIND_ADDRESS_VARIABLE).unwrap_or_else(|| String::from(DEFAULT_BIND_ADDRESS));
        let bind_address: IpAddr = parse_variable(BIND_ADDRESS_VARIABLE, &bind_address_text)?;
        let port: u16 = match get_set_variable(PORT_VARIABLE) {
            Some(port_text) => parse_variable(PORT_VARIABLE, &port_text)?,
            None => DEFAULT_PORT,
        };
        let allowed_origins_text: String =
            get_set_variable(ALLOWED_ORIGINS_VARIABLE).unwrap_or_else(|| String::from(DEFAULT_ALLOWED_ORIGINS));
        let tls_files: Option<TlsFiles> = get_tls_files(
            get_set_variable(TLS_CERTIFICATE_PATH_VARIABLE),
            get_set_variable(TLS_PRIVATE_KEY_PATH_VARIABLE),
        )?;

        Ok(ServerConfig {
            bind_address,
            port,
            site_root: get_set_variable(SITE_ROOT_VARIABLE).map(PathBuf::from),
            allowed_origins: split_origins(&allowed_origins_text),
            replay_capture_directory: get_set_variable(REPLAY_CAPTURE_DIRECTORY_VARIABLE).map(PathBuf::from),
            tls_files,
            empty_game_grace_period: EMPTY_GAME_GRACE_PERIOD,
            tick_period: Duration::from_millis(u64::from(tick::TICK_PERIOD_MILLISECONDS)),
        })
    }
}

/// PEM files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TlsFiles {
    pub certificate_chain_path: PathBuf,
    pub private_key_path: PathBuf,
}

/// Exact match.
pub fn is_origin_allowed(allowed_origins: &[String], origin: &str) -> bool {
    allowed_origins.iter().any(|allowed_origin: &String| allowed_origin == origin)
}

fn get_tls_files(
    certificate_path: Option<String>,
    private_key_path: Option<String>,
) -> Result<Option<TlsFiles>, AppError> {
    match (certificate_path, private_key_path) {
        (Some(certificate_path), Some(private_key_path)) => Ok(Some(TlsFiles {
            certificate_chain_path: PathBuf::from(certificate_path),
            private_key_path: PathBuf::from(private_key_path),
        })),
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => Err(AppError::new(&format!(
            "set both {TLS_CERTIFICATE_PATH_VARIABLE} and {TLS_PRIVATE_KEY_PATH_VARIABLE}, or neither"
        ))),
    }
}

fn parse_variable<T: FromStr>(name: &str, value: &str) -> Result<T, AppError> {
    value.trim().parse::<T>().map_err(|_| AppError::new(&format!("{name} is not valid: {value}")))
}

/// Trailing slashes are dropped; a browser's `Origin` has none.
fn split_origins(allowed_origins_text: &str) -> Vec<String> {
    allowed_origins_text
        .split(',')
        .map(|origin: &str| origin.trim().trim_end_matches('/'))
        .filter(|origin: &&str| !origin.is_empty())
        .map(String::from)
        .collect()
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib config::config`

Expected: `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/lib.rs server/src/config/mod.rs server/src/config/config.rs
git commit -m "server config"
```

### Task 4: Optional TLS

**Files:**
- Create: `server/src/config/tls.rs`
- Create: `server/tests/fixtures/localhost.certificate.pem`, `server/tests/fixtures/localhost.key.pem` (generated)
- Modify: `server/src/config/mod.rs`
- Test: inline `mod tests` in `server/src/config/tls.rs`

rustls inside actix-web (server.md, process): the chain through `CertificateDer::pem_file_iter`, the key through `PrivateKeyDer::from_pem_file`, then `with_no_client_auth().with_single_cert`. rustls resolves to its `aws-lc-rs` provider alone, so `rustls::ServerConfig::builder()` needs no explicit provider.

- [ ] **Step 1: Generate the test certificate**

Run: `mkdir -p server/tests/fixtures && openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 36500 -subj /CN=localhost -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" -keyout server/tests/fixtures/localhost.key.pem -out server/tests/fixtures/localhost.certificate.pem`

Expected: exit status 0; `server/tests/fixtures/localhost.certificate.pem` starts with `-----BEGIN CERTIFICATE-----` and `server/tests/fixtures/localhost.key.pem` with `-----BEGIN PRIVATE KEY-----`.

- [ ] **Step 2: Write the failing test**

Create `server/src/config/tls.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn get_fixture_path(file_name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(file_name)
    }

    #[test]
    fn create_rustls_config_reads_the_certificate_chain_and_private_key() {
        let tls_files: TlsFiles = TlsFiles {
            certificate_chain_path: get_fixture_path("localhost.certificate.pem"),
            private_key_path: get_fixture_path("localhost.key.pem"),
        };

        assert!(create_rustls_config(&tls_files).is_ok());
    }

    #[test]
    fn create_rustls_config_rejects_a_missing_file() {
        let tls_files: TlsFiles = TlsFiles {
            certificate_chain_path: get_fixture_path("missing.pem"),
            private_key_path: get_fixture_path("localhost.key.pem"),
        };

        assert!(create_rustls_config(&tls_files).is_err());
    }

    #[test]
    fn create_rustls_config_rejects_a_key_as_the_certificate_chain() {
        let tls_files: TlsFiles = TlsFiles {
            certificate_chain_path: get_fixture_path("localhost.key.pem"),
            private_key_path: get_fixture_path("localhost.key.pem"),
        };

        assert!(create_rustls_config(&tls_files).is_err());
    }
}
```

Replace the whole contents of `server/src/config/mod.rs` with:

```rust
pub mod config;
pub mod tls;

pub use config::*;
pub use tls::*;
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p server --lib config::tls`

Expected: compile failure, first line ``error[E0425]: cannot find type `TlsFiles` in this scope``.

- [ ] **Step 4: Implement**

In `server/src/config/tls.rs`, replace everything above `#[cfg(test)]` with:

```rust
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use shared::error::AppError;

use crate::config::TlsFiles;

pub fn create_rustls_config(tls_files: &TlsFiles) -> Result<rustls::ServerConfig, AppError> {
    let certificate_chain: Vec<CertificateDer<'static>> = read_certificate_chain(tls_files)?;
    let private_key: PrivateKeyDer<'static> =
        PrivateKeyDer::from_pem_file(&tls_files.private_key_path).map_err(|error| {
            AppError::from_error(
                &format!("cannot read the private key {}", tls_files.private_key_path.display()),
                Box::new(error),
            )
        })?;

    rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificate_chain, private_key)
        .map_err(|error| AppError::from_error("the certificate chain does not match the private key", Box::new(error)))
}

fn read_certificate_chain(tls_files: &TlsFiles) -> Result<Vec<CertificateDer<'static>>, AppError> {
    let certificate_error = |error: rustls::pki_types::pem::Error| {
        AppError::from_error(
            &format!(
                "cannot read the certificate chain {}",
                tls_files.certificate_chain_path.display()
            ),
            Box::new(error),
        )
    };

    let certificate_chain: Vec<CertificateDer<'static>> =
        CertificateDer::pem_file_iter(&tls_files.certificate_chain_path)
            .map_err(certificate_error)?
            .collect::<Result<Vec<CertificateDer<'static>>, rustls::pki_types::pem::Error>>()
            .map_err(certificate_error)?;

    if certificate_chain.is_empty() {
        return Err(AppError::new(&format!(
            "the certificate chain {} holds no certificate",
            tls_files.certificate_chain_path.display()
        )));
    }

    Ok(certificate_chain)
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p server --lib config::tls`

Expected: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 6 filtered out`.

- [ ] **Step 6: Commit**

```sh
git status
git add server/src/config/mod.rs server/src/config/tls.rs server/tests/fixtures/localhost.certificate.pem server/tests/fixtures/localhost.key.pem
git commit -m "optional tls config"
```

### Task 5: Token bucket rate limit

**Files:**
- Create: `server/src/socket/mod.rs`
- Create: `server/src/socket/rate_limit.rs`
- Modify: `server/src/lib.rs`
- Test: inline `mod tests` in `server/src/socket/rate_limit.rs`

Per connection: inputs 30/s with a burst of 45, requests 4/s with a burst of 8; a bucket which refuses for 10 s without taking a token reports abuse (server.md, rate limits). Floats are acceptable outside the simulation.

- [ ] **Step 1: Write the failing test**

Create `server/src/socket/rate_limit.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn take_repeatedly(bucket: &mut TokenBucket, take_count: u32, now: Instant) -> Vec<TokenTakeKind> {
        (0..take_count).map(|_| bucket.take(now)).collect()
    }

    #[test]
    fn take_allows_the_burst_then_refuses() {
        let now: Instant = Instant::now();
        let mut bucket: TokenBucket = TokenBucket::create_request_bucket(now);

        let take_kinds: Vec<TokenTakeKind> = take_repeatedly(&mut bucket, 9, now);

        assert_eq!(take_kinds[..8], [TokenTakeKind::Taken; 8]);
        assert_eq!(take_kinds[8], TokenTakeKind::Refused);
    }

    #[test]
    fn take_refills_at_the_rate() {
        let start: Instant = Instant::now();
        let mut bucket: TokenBucket = TokenBucket::create_request_bucket(start);
        take_repeatedly(&mut bucket, 8, start);

        let after_half_a_second: Instant = start + Duration::from_millis(500);

        assert_eq!(
            take_repeatedly(&mut bucket, 3, after_half_a_second),
            vec![TokenTakeKind::Taken, TokenTakeKind::Taken, TokenTakeKind::Refused],
        );
    }

    #[test]
    fn take_refills_no_further_than_the_burst() {
        let start: Instant = Instant::now();
        let mut bucket: TokenBucket = TokenBucket::create_input_bucket(start);

        let take_kinds: Vec<TokenTakeKind> = take_repeatedly(&mut bucket, 46, start + Duration::from_secs(60));

        assert_eq!(take_kinds[..45], [TokenTakeKind::Taken; 45]);
        assert_eq!(take_kinds[45], TokenTakeKind::Refused);
    }

    #[test]
    fn take_reports_abuse_after_refusing_for_the_abuse_period() {
        let start: Instant = Instant::now();
        let mut bucket: TokenBucket = TokenBucket::new(0.0, 1.0, start);
        bucket.take(start);

        assert_eq!(bucket.take(start + Duration::from_secs(1)), TokenTakeKind::Refused);
        assert_eq!(
            bucket.take(start + Duration::from_millis(10_999)),
            TokenTakeKind::Refused
        );
        assert_eq!(bucket.take(start + Duration::from_secs(11)), TokenTakeKind::Abused);
    }

    #[test]
    fn take_forgets_refusals_once_a_token_is_taken() {
        let start: Instant = Instant::now();
        let mut bucket: TokenBucket = TokenBucket::new(0.1, 1.0, start);
        bucket.take(start);

        assert_eq!(bucket.take(start + Duration::from_secs(1)), TokenTakeKind::Refused);
        assert_eq!(bucket.take(start + Duration::from_millis(10_500)), TokenTakeKind::Taken);
        assert_eq!(bucket.take(start + Duration::from_secs(12)), TokenTakeKind::Refused);
    }
}
```

Create `server/src/socket/mod.rs`:

```rust
pub mod rate_limit;

pub use rate_limit::*;
```

Replace the whole contents of `server/src/lib.rs` with:

```rust
//! The game server.

pub mod config;
pub mod socket;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib socket::rate_limit`

Expected: compile failure, first line ``error[E0425]: cannot find type `TokenBucket` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/socket/rate_limit.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::time::{Duration, Instant};

pub const INPUT_RATE_PER_SECOND: f64 = 30.0;
pub const INPUT_BURST: f64 = 45.0;
pub const REQUEST_RATE_PER_SECOND: f64 = 4.0;
pub const REQUEST_BURST: f64 = 8.0;
pub const RATE_LIMIT_ABUSE_PERIOD: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TokenBucket {
    rate_per_second: f64,
    burst: f64,
    tokens: f64,
    refilled_at: Instant,
    /// Since the first refusal after the last token taken.
    refusing_since: Option<Instant>,
}

impl TokenBucket {
    /// Starts full.
    pub fn new(rate_per_second: f64, burst: f64, now: Instant) -> TokenBucket {
        TokenBucket {
            rate_per_second,
            burst,
            tokens: burst,
            refilled_at: now,
            refusing_since: None,
        }
    }

    pub fn create_input_bucket(now: Instant) -> TokenBucket {
        TokenBucket::new(INPUT_RATE_PER_SECOND, INPUT_BURST, now)
    }

    pub fn create_request_bucket(now: Instant) -> TokenBucket {
        TokenBucket::new(REQUEST_RATE_PER_SECOND, REQUEST_BURST, now)
    }

    pub fn take(&mut self, now: Instant) -> TokenTakeKind {
        self.refill(now);

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            self.refusing_since = None;

            return TokenTakeKind::Taken;
        }

        let refusing_since: Instant = *self.refusing_since.get_or_insert(now);

        if now.duration_since(refusing_since) >= RATE_LIMIT_ABUSE_PERIOD {
            return TokenTakeKind::Abused;
        }

        TokenTakeKind::Refused
    }

    fn refill(&mut self, now: Instant) {
        let elapsed_seconds: f64 = now.saturating_duration_since(self.refilled_at).as_secs_f64();
        self.tokens = (self.tokens + elapsed_seconds * self.rate_per_second).min(self.burst);
        self.refilled_at = now;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenTakeKind {
    Taken,
    Refused,
    /// Refused without a token taken for the whole abuse period.
    Abused,
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib socket::rate_limit`

Expected: `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/lib.rs server/src/socket/mod.rs server/src/socket/rate_limit.rs
git commit -m "token bucket rate limit"
```

### Task 6: Cursor clamp

**Files:**
- Create: `server/src/game/mod.rs`
- Create: `server/src/game/cursor_clamp.rs`
- Modify: `server/src/lib.rs`
- Test: inline `mod tests` in `server/src/game/cursor_clamp.rs`

server.md (input validation and speed clamp): within the budget the target applies and the distance is spent; beyond it the cursor moves towards the target by the whole budget. Distances are in `i64`, so opposite corners of the `±2^28` coordinate limit cannot overflow.

- [ ] **Step 1: Write the failing test**

Create `server/src/game/cursor_clamp.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shared::protocol::protocol_limits;

    fn create_clamp_with_budget_ticks(budget_tick_count: u32) -> CursorClamp {
        let mut clamp: CursorClamp = CursorClamp::new(SubpixelPoint { x: 0, y: 0 });

        for _ in 0..budget_tick_count {
            clamp.add_tick_budget();
        }

        clamp
    }

    #[test]
    fn apply_moves_to_a_target_within_the_budget_and_spends_the_distance() {
        let mut clamp: CursorClamp = create_clamp_with_budget_ticks(1);

        assert_eq!(
            clamp.apply(SubpixelPoint { x: 3000, y: 0 }),
            CursorClampKind::WithinBudget
        );
        assert_eq!(clamp.applied_cursor(), SubpixelPoint { x: 3000, y: 0 });
        assert_eq!(clamp.apply(SubpixelPoint { x: 3200, y: 0 }), CursorClampKind::Clamped);
        assert_eq!(clamp.applied_cursor(), SubpixelPoint { x: 3174, y: 0 });
    }

    #[test]
    fn apply_moves_along_the_direction_by_the_budget_beyond_it() {
        let mut clamp: CursorClamp = create_clamp_with_budget_ticks(1);

        assert_eq!(
            clamp.apply(SubpixelPoint { x: 30_000, y: -40_000 }),
            CursorClampKind::Clamped
        );
        assert_eq!(clamp.applied_cursor(), SubpixelPoint { x: 1904, y: -2539 });
        assert_eq!(
            clamp.apply(SubpixelPoint { x: 1905, y: -2539 }),
            CursorClampKind::Clamped
        );
        assert_eq!(clamp.applied_cursor(), SubpixelPoint { x: 1904, y: -2539 });
    }

    #[test]
    fn add_tick_budget_stops_at_the_cap() {
        let mut clamp: CursorClamp = create_clamp_with_budget_ticks(20);

        assert_eq!(
            clamp.apply(SubpixelPoint { x: 100_000, y: 0 }),
            CursorClampKind::Clamped
        );
        assert_eq!(clamp.applied_cursor(), SubpixelPoint { x: 47_610, y: 0 });
    }

    #[test]
    fn new_starts_at_the_spawn_cursor_with_an_empty_budget() {
        let spawn_cursor: SubpixelPoint = SubpixelPoint { x: 51_200, y: 102_400 };
        let mut clamp: CursorClamp = CursorClamp::new(spawn_cursor);

        assert_eq!(clamp.apply(spawn_cursor), CursorClampKind::WithinBudget);
        assert_eq!(
            clamp.apply(SubpixelPoint { x: 51_201, y: 102_400 }),
            CursorClampKind::Clamped
        );
        assert_eq!(clamp.applied_cursor(), spawn_cursor);
    }

    #[test]
    fn apply_handles_opposite_corners_of_the_coordinate_limit() {
        let limit: i32 = protocol_limits::COORDINATE_LIMIT_SUBPIXELS;
        let mut clamp: CursorClamp = CursorClamp::new(SubpixelPoint { x: -limit, y: -limit });

        for _ in 0..15 {
            clamp.add_tick_budget();
        }

        assert_eq!(
            clamp.apply(SubpixelPoint { x: limit, y: limit }),
            CursorClampKind::Clamped
        );
        assert_eq!(
            clamp.applied_cursor(),
            SubpixelPoint {
                x: -limit + 33_665,
                y: -limit + 33_665,
            },
        );
    }
}
```

Create `server/src/game/mod.rs`:

```rust
pub mod cursor_clamp;

pub use cursor_clamp::*;
```

Replace the whole contents of `server/src/lib.rs` with:

```rust
//! The game server.

pub mod config;
pub mod game;
pub mod socket;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib game::cursor_clamp`

Expected: compile failure, first line ``error[E0425]: cannot find type `CursorClamp` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/game/cursor_clamp.rs`, replace everything above `#[cfg(test)]` with:

```rust
use shared::geometry::SubpixelPoint;

/// 42.5 px/s is 2.975 px, or 3046.4 subpixels, per tick.
pub const MOVE_SUBPIXELS_PER_TICK: i64 = 3046;
pub const MOVE_TOLERANCE_SUBPIXELS_PER_TICK: i64 = 128;
pub const MOVE_BUDGET_CAP_TICKS: i64 = 15;
const MOVE_BUDGET_SUBPIXELS_PER_TICK: i64 = MOVE_SUBPIXELS_PER_TICK + MOVE_TOLERANCE_SUBPIXELS_PER_TICK;
const MOVE_BUDGET_CAP_SUBPIXELS: i64 = MOVE_BUDGET_CAP_TICKS * MOVE_BUDGET_SUBPIXELS_PER_TICK;

/// Holds one organism's cursor to the movement speed, with a catch-up allowance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorClamp {
    applied_cursor: SubpixelPoint,
    budget_subpixels: i64,
}

impl CursorClamp {
    /// With an empty budget.
    pub fn new(spawn_cursor: SubpixelPoint) -> CursorClamp {
        CursorClamp {
            applied_cursor: spawn_cursor,
            budget_subpixels: 0,
        }
    }

    pub fn applied_cursor(&self) -> SubpixelPoint {
        self.applied_cursor
    }

    pub fn add_tick_budget(&mut self) {
        self.budget_subpixels = (self.budget_subpixels + MOVE_BUDGET_SUBPIXELS_PER_TICK).min(MOVE_BUDGET_CAP_SUBPIXELS);
    }

    /// Beyond the budget the cursor moves towards `target` by the whole budget.
    pub fn apply(&mut self, target: SubpixelPoint) -> CursorClampKind {
        let dx: i64 = i64::from(target.x) - i64::from(self.applied_cursor.x);
        let dy: i64 = i64::from(target.y) - i64::from(self.applied_cursor.y);
        let distance_squared: i64 = dx * dx + dy * dy;

        if distance_squared <= self.budget_subpixels * self.budget_subpixels {
            let distance: i64 = (distance_squared as f64).sqrt().ceil() as i64;
            self.applied_cursor = target;
            self.budget_subpixels = (self.budget_subpixels - distance).max(0);

            return CursorClampKind::WithinBudget;
        }

        let scale: f64 = self.budget_subpixels as f64 / (distance_squared as f64).sqrt();
        self.applied_cursor = SubpixelPoint {
            x: get_moved_coordinate(self.applied_cursor.x, dx, scale),
            y: get_moved_coordinate(self.applied_cursor.y, dy, scale),
        };
        self.budget_subpixels = 0;

        CursorClampKind::Clamped
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorClampKind {
    WithinBudget,
    Clamped,
}

/// Lies between the coordinate and its target, so within the coordinate limit.
fn get_moved_coordinate(coordinate: i32, difference: i64, scale: f64) -> i32 {
    let moved_coordinate: i64 = i64::from(coordinate) + (difference as f64 * scale).round() as i64;

    i32::try_from(moved_coordinate).unwrap_or(coordinate)
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib game::cursor_clamp`

Expected: `test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/lib.rs server/src/game/mod.rs server/src/game/cursor_clamp.rs
git commit -m "cursor clamp"
```

### Task 7: Game runtime

**Files:**
- Create: `server/src/game/game_runtime.rs`
- Modify: `server/src/game/mod.rs`
- Test: inline `mod tests` in `server/src/game/game_runtime.rs`

Game tasks, supervisors, replay writers and the lobby task run on one multi-threaded runtime, never on an actix worker (server.md, game task). `get_available_cpu_count` also sizes the password semaphore (Task 9).

- [ ] **Step 1: Write the failing test**

Create `server/src/game/game_runtime.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::task::JoinHandle;

    #[test]
    fn game_runtime_runs_a_spawned_task_with_timers() {
        let join_handle: JoinHandle<u32> = GAME_RUNTIME.spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;

            7
        });

        assert_eq!(GAME_RUNTIME.block_on(join_handle).unwrap(), 7);
    }
}
```

Replace the whole contents of `server/src/game/mod.rs` with:

```rust
pub mod cursor_clamp;
pub mod game_runtime;

pub use cursor_clamp::*;
pub use game_runtime::*;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib game::game_runtime`

Expected: compile failure, first line ``error[E0425]: cannot find value `GAME_RUNTIME` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/game/game_runtime.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::num::NonZeroUsize;
use std::sync::LazyLock;

use tokio::runtime::Runtime;

pub static GAME_RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(get_available_cpu_count())
        .thread_name("game")
        .enable_all()
        .build()
        .expect("the game runtime starts")
});

/// At least one.
pub fn get_available_cpu_count() -> usize {
    std::thread::available_parallelism().map(NonZeroUsize::get).unwrap_or(1)
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib game::game_runtime`

Expected: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 19 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/game/mod.rs server/src/game/game_runtime.rs
git commit -m "game runtime"
```

### Task 8: Per-IP password failure window

**Files:**
- Create: `server/src/password/mod.rs`
- Create: `server/src/password/password_failure_limit.rs`
- Modify: `server/src/lib.rs`
- Test: inline `mod tests` in `server/src/password/password_failure_limit.rs`

20 failed password attempts per IP per 10 minutes (protocol.md, limits); further attempts get `RateLimited` without hashing. A sliding window of failure instants per IP, emptied entries removed; `forget_expired_failures` sweeps IPs which never attempt again, called by the lobby task (Task 22). The functions take the map, so `ServerRegistries` (Task 11) can hand in a leaked one.

- [ ] **Step 1: Write the failing test**

Create `server/src/password/password_failure_limit.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7));
    const OTHER_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 8));

    fn record_failures(failures: &DashMap<IpAddr, PasswordFailureWindow>, failure_count: u32, now: Instant) {
        for _ in 0..failure_count {
            record_failure(failures, IP, now);
        }
    }

    #[test]
    fn check_attempt_allows_an_ip_below_the_failure_limit() {
        let failures: DashMap<IpAddr, PasswordFailureWindow> = DashMap::new();
        let now: Instant = Instant::now();
        record_failures(&failures, 19, now);

        assert_eq!(check_attempt(&failures, IP, now), PasswordAttemptKind::Allowed);
    }

    #[test]
    fn check_attempt_limits_an_ip_at_the_failure_limit_only() {
        let failures: DashMap<IpAddr, PasswordFailureWindow> = DashMap::new();
        let now: Instant = Instant::now();
        record_failures(&failures, 20, now);

        assert_eq!(check_attempt(&failures, IP, now), PasswordAttemptKind::RateLimited);
        assert_eq!(check_attempt(&failures, OTHER_IP, now), PasswordAttemptKind::Allowed);
    }

    #[test]
    fn check_attempt_forgets_failures_older_than_the_window() {
        let failures: DashMap<IpAddr, PasswordFailureWindow> = DashMap::new();
        let start: Instant = Instant::now();
        record_failures(&failures, 10, start);
        record_failures(&failures, 10, start + Duration::from_secs(300));

        assert_eq!(
            check_attempt(&failures, IP, start + Duration::from_secs(599)),
            PasswordAttemptKind::RateLimited
        );
        assert_eq!(
            check_attempt(&failures, IP, start + Duration::from_secs(600)),
            PasswordAttemptKind::Allowed
        );
    }

    #[test]
    fn check_attempt_removes_an_ip_whose_failures_all_expired() {
        let failures: DashMap<IpAddr, PasswordFailureWindow> = DashMap::new();
        let start: Instant = Instant::now();
        record_failures(&failures, 1, start);

        check_attempt(&failures, IP, start + Duration::from_secs(600));

        assert!(failures.is_empty());
    }
}
```

Create `server/src/password/mod.rs`:

```rust
pub mod password_failure_limit;

pub use password_failure_limit::*;
```

Replace the whole contents of `server/src/lib.rs` with:

```rust
//! The game server.

pub mod config;
pub mod game;
pub mod password;
pub mod socket;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib password::password_failure_limit`

Expected: compile failure, first line ``error[E0425]: cannot find type `IpAddr` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/password/password_failure_limit.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::collections::VecDeque;
use std::net::IpAddr;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use dashmap::mapref::one::RefMut;
use shared::protocol::protocol_limits;

pub static PASSWORD_FAILURES_PER_IP: LazyLock<DashMap<IpAddr, PasswordFailureWindow>> = LazyLock::new(DashMap::new);

const PASSWORD_FAILURE_WINDOW: Duration = Duration::from_secs(protocol_limits::PASSWORD_FAILURE_WINDOW_SECONDS);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PasswordFailureWindow {
    /// Oldest first.
    failed_at: VecDeque<Instant>,
}

impl PasswordFailureWindow {
    fn forget_expired(&mut self, now: Instant) {
        while let Some(oldest_failed_at) = self.failed_at.front().copied() {
            if now.saturating_duration_since(oldest_failed_at) < PASSWORD_FAILURE_WINDOW {
                break;
            }

            self.failed_at.pop_front();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordAttemptKind {
    Allowed,
    RateLimited,
}

pub fn check_attempt(
    password_failures_per_ip: &DashMap<IpAddr, PasswordFailureWindow>,
    ip: IpAddr,
    now: Instant,
) -> PasswordAttemptKind {
    let Entry::Occupied(mut occupied_entry) = password_failures_per_ip.entry(ip) else {
        return PasswordAttemptKind::Allowed;
    };

    occupied_entry.get_mut().forget_expired(now);
    let failure_count: u32 = u32::try_from(occupied_entry.get().failed_at.len()).unwrap_or(u32::MAX);

    if failure_count == 0 {
        occupied_entry.remove();
    }

    if failure_count >= protocol_limits::PASSWORD_FAILURE_LIMIT_PER_IP {
        return PasswordAttemptKind::RateLimited;
    }

    PasswordAttemptKind::Allowed
}

pub fn record_failure(password_failures_per_ip: &DashMap<IpAddr, PasswordFailureWindow>, ip: IpAddr, now: Instant) {
    let mut window: RefMut<'_, IpAddr, PasswordFailureWindow> = password_failures_per_ip.entry(ip).or_default();
    window.forget_expired(now);
    window.failed_at.push_back(now);
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib password::password_failure_limit`

Expected: `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 20 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/lib.rs server/src/password/mod.rs server/src/password/password_failure_limit.rs
git commit -m "per ip password failure window"
```

### Task 9: Password hashing

**Files:**
- Create: `server/src/password/password.rs`
- Modify: `server/src/password/mod.rs`
- Test: inline `mod tests` in `server/src/password/password.rs`

bcrypt cost 10 on `web::block`, bounded by `PASSWORD_CHECK_SEMAPHORE` with one permit per CPU; `try_acquire` failing answers `ServerBusy` (server.md, passwords). `web::block` needs an actix system, hence `#[actix_web::test]`. Cost 10 takes about 150 ms per hash in a debug build.

- [ ] **Step 1: Write the failing test**

Create `server/src/password/password.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn verify_password_matches_only_the_hashed_password() {
        let password_hash: String = hash_password(String::from("open sesame")).await.unwrap();

        assert_ne!(password_hash, "open sesame");
        assert_eq!(
            verify_password(String::from("open sesame"), password_hash.clone()).await,
            Ok(PasswordMatchKind::Matches),
        );
        assert_eq!(
            verify_password(String::from("open sesame "), password_hash).await,
            Ok(PasswordMatchKind::DoesNotMatch),
        );
    }

    #[actix_web::test]
    async fn run_password_check_is_busy_without_a_free_permit() {
        let semaphore: Semaphore = Semaphore::new(0);

        assert_eq!(
            run_password_check(&semaphore, || Ok(1)).await,
            Err(RejectionKind::ServerBusy)
        );
    }

    #[actix_web::test]
    async fn run_password_check_is_busy_when_the_check_fails() {
        let semaphore: Semaphore = Semaphore::new(1);

        assert_eq!(
            run_password_check(&semaphore, || Err::<u32, AppErrorStatic>(AppErrorStatic::new("broken"))).await,
            Err(RejectionKind::ServerBusy),
        );
        assert_eq!(semaphore.available_permits(), 1);
    }
}
```

Replace the whole contents of `server/src/password/mod.rs` with:

```rust
pub mod password;
pub mod password_failure_limit;

pub use password::*;
pub use password_failure_limit::*;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib password::password::`

Expected: compile failure, first line ``error[E0425]: cannot find type `Semaphore` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/password/password.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::sync::LazyLock;

use actix_web::error::BlockingError;
use actix_web::web;
use tokio::sync::{Semaphore, SemaphorePermit};

use shared::error::{AppError, AppErrorStatic};
use shared::protocol::RejectionKind;

use crate::game::game_runtime;

static PASSWORD_CHECK_SEMAPHORE: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(game_runtime::get_available_cpu_count()));

/// Singularity's cost.
const BCRYPT_COST: u32 = 10;
/// Per connection.
pub const PASSWORD_FAILURE_LIMIT: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordMatchKind {
    Matches,
    DoesNotMatch,
}

pub async fn hash_password(password: String) -> Result<String, RejectionKind> {
    run_password_check(&PASSWORD_CHECK_SEMAPHORE, move || {
        bcrypt::hash(password, BCRYPT_COST)
            .map_err(|error| AppErrorStatic::from(AppError::from_error("cannot hash the password", Box::new(error))))
    })
    .await
}

pub async fn verify_password(password: String, password_hash: String) -> Result<PasswordMatchKind, RejectionKind> {
    run_password_check(&PASSWORD_CHECK_SEMAPHORE, move || {
        let matches: bool = bcrypt::verify(password, &password_hash).map_err(|error| {
            AppErrorStatic::from(AppError::from_error("cannot verify the password", Box::new(error)))
        })?;

        match matches {
            true => Ok(PasswordMatchKind::Matches),
            false => Ok(PasswordMatchKind::DoesNotMatch),
        }
    })
    .await
}

/// `ServerBusy` when every permit is taken or the check fails.
async fn run_password_check<T: Send + 'static>(
    semaphore: &Semaphore,
    check: impl FnOnce() -> Result<T, AppErrorStatic> + Send + 'static,
) -> Result<T, RejectionKind> {
    let Ok(_permit): Result<SemaphorePermit<'_>, _> = semaphore.try_acquire() else {
        return Err(RejectionKind::ServerBusy);
    };

    let check_result: Result<Result<T, AppErrorStatic>, BlockingError> = web::block(check).await;

    match check_result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => {
            log::error!("password check failed; [error={error}]");

            Err(RejectionKind::ServerBusy)
        }
        Err(error) => {
            log::error!("password check did not run; [error={error}]");

            Err(RejectionKind::ServerBusy)
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib password::password::`

Expected: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 24 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/password/mod.rs server/src/password/password.rs
git commit -m "password hashing"
```

### Task 10: Connection model and game handle

**Files:**
- Create: `server/src/socket/connection_model.rs`
- Create: `server/src/game/game_handle.rs`
- Modify: `server/src/socket/mod.rs`
- Modify: `server/src/game/mod.rs`
- Test: inline `mod tests` in `server/src/socket/connection_model.rs`

The types every later module shares (server.md, registries and tasks and channels): the connection's phase and control, the close causes of the decisions above, a membership's two ends (`ConnectionEndpoint` sends, owned by the game task; `Membership` receives, kept by the connection), and `GameHandle`, `GameRequest`, `GameInput`.

- [ ] **Step 1: Write the failing test**

Create `server/src/socket/connection_model.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_code_maps_every_cause() {
        assert_eq!(CloseCauseKind::Reason(CloseReasonKind::SlowConsumer).close_code(), 4002);
        assert_eq!(
            CloseCauseKind::ProtocolViolation(ProtocolViolationKind::TextFrame).close_code(),
            1008
        );
        assert_eq!(CloseCauseKind::ClientClosed.close_code(), 1000);
    }

    #[test]
    fn description_names_the_protocol_violation_cause() {
        assert_eq!(
            CloseCauseKind::ProtocolViolation(ProtocolViolationKind::Oversize).description(),
            Some(String::from("oversize")),
        );
        assert_eq!(
            CloseCauseKind::Reason(CloseReasonKind::InboundTimeout).description(),
            Some(String::from("inbound timeout")),
        );
        assert_eq!(
            CloseCauseKind::Reason(CloseReasonKind::InternalError).description(),
            None
        );
        assert_eq!(CloseCauseKind::ClientClosed.description(), None);
    }

    #[test]
    fn create_membership_connects_the_endpoint_to_the_membership() {
        let (endpoint, mut membership): (ConnectionEndpoint, Membership) = create_membership(ConnectionId(3));

        endpoint.outbound.try_send(Bytes::from_static(b"frame")).unwrap();
        endpoint.control.send(ConnectionControl::Close(CloseCauseKind::ClientClosed)).unwrap();

        assert_eq!(endpoint.connection_id, ConnectionId(3));
        assert_eq!(membership.outbound.try_recv().unwrap(), Bytes::from_static(b"frame"));
        assert_eq!(
            *membership.control.borrow(),
            ConnectionControl::Close(CloseCauseKind::ClientClosed)
        );
    }

    #[test]
    fn encode_frame_encodes_the_message() {
        let frame: Bytes = encode_frame(&MessageSerialOut::LeftGame);

        assert_eq!(
            protocol::decode_message_out(&frame).unwrap(),
            MessageSerialOut::LeftGame
        );
    }
}
```

Replace the whole contents of `server/src/socket/mod.rs` with:

```rust
pub mod connection_model;
pub mod rate_limit;

pub use connection_model::*;
pub use rate_limit::*;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib socket::connection_model`

Expected: compile failure, first line ``error[E0425]: cannot find type `ConnectionEndpoint` in this scope``.

- [ ] **Step 3: Implement**

Create `server/src/game/game_handle.rs`:

```rust
use std::time::Duration;

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot, watch};

use shared::ability::Loadout;
use shared::game::{GameSettings, GameSummary, PlayerInput};
use shared::member::{Appearance, Joiner, MemberId, TeamChoiceKind};
use shared::protocol::RejectionKind;

use crate::socket::{ConnectionControl, ConnectionId};

pub const GAME_REQUEST_CHANNEL_CAPACITY: usize = 64;
pub const GAME_INPUT_CHANNEL_CAPACITY: usize = 1024;
/// For join, spectate, respawn and appearance requests.
pub const REQUEST_SEND_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct GameHandle {
    pub requests: mpsc::Sender<GameRequest>,
    pub inputs: mpsc::Sender<GameInput>,
    pub summary: watch::Receiver<GameSummary>,
    pub password_hash: Option<String>,
    pub settings: GameSettings,
}

pub enum GameRequest {
    Join {
        endpoint: ConnectionEndpoint,
        joiner: Joiner,
        reply: oneshot::Sender<Result<MemberId, RejectionKind>>,
    },
    Spectate {
        endpoint: ConnectionEndpoint,
        screen_name: String,
        reply: oneshot::Sender<Result<MemberId, RejectionKind>>,
    },
    Respawn {
        member_id: MemberId,
        loadout: Loadout,
        team: TeamChoiceKind,
        reply: oneshot::Sender<Result<(), RejectionKind>>,
    },
    UpdateAppearance {
        member_id: MemberId,
        appearance: Appearance,
    },
    Leave {
        member_id: MemberId,
        reply: oneshot::Sender<()>,
    },
    ConnectionClosed {
        connection_id: ConnectionId,
    },
    RequestSnapshot {
        member_id: MemberId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameInput {
    pub member_id: MemberId,
    pub input: PlayerInput,
}

/// The sending ends of a membership, owned by the game task.
pub struct ConnectionEndpoint {
    pub connection_id: ConnectionId,
    pub outbound: mpsc::Sender<Bytes>,
    pub control: watch::Sender<ConnectionControl>,
}
```

Replace the whole contents of `server/src/game/mod.rs` with:

```rust
pub mod cursor_clamp;
pub mod game_handle;
pub mod game_runtime;

pub use cursor_clamp::*;
pub use game_handle::*;
pub use game_runtime::*;
```

In `server/src/socket/connection_model.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::sync::atomic::AtomicU64;

use bytes::Bytes;
use tokio::sync::{mpsc, watch};

use shared::game::GameId;
use shared::member::MemberId;
use shared::protocol;
use shared::protocol::{CloseReasonKind, MessageSerialOut};

use crate::game::ConnectionEndpoint;

pub const FIRST_CONNECTION_ID: u64 = 1;
pub const MEMBERSHIP_CHANNEL_CAPACITY: usize = 256;
pub const NORMAL_CLOSURE_CODE: u16 = 1000;

pub static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(FIRST_CONNECTION_ID);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionPhase {
    Lobby,
    InGame { game_id: GameId, member_id: MemberId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionControl {
    Open,
    Close(CloseCauseKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolViolationKind {
    Decode,
    Conversion,
    TextFrame,
    Oversize,
    Validation,
}

impl ProtocolViolationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ProtocolViolationKind::Decode => "decode",
            ProtocolViolationKind::Conversion => "conversion",
            ProtocolViolationKind::TextFrame => "text frame",
            ProtocolViolationKind::Oversize => "oversize",
            ProtocolViolationKind::Validation => "validation",
        }
    }
}

/// Why the server closes a connection, with the cause of a protocol violation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseCauseKind {
    Reason(CloseReasonKind),
    ProtocolViolation(ProtocolViolationKind),
    /// Answers the client's own close.
    ClientClosed,
}

impl CloseCauseKind {
    pub fn close_code(self) -> u16 {
        match self {
            CloseCauseKind::Reason(close_reason) => close_reason.close_code(),
            CloseCauseKind::ProtocolViolation(_) => CloseReasonKind::ProtocolViolation.close_code(),
            CloseCauseKind::ClientClosed => NORMAL_CLOSURE_CODE,
        }
    }

    pub fn description(self) -> Option<String> {
        match self {
            CloseCauseKind::Reason(close_reason) => close_reason.reason_text(),
            CloseCauseKind::ProtocolViolation(violation) => Some(String::from(violation.as_str())),
            CloseCauseKind::ClientClosed => None,
        }
    }
}

/// The receiving ends of a membership, kept by the connection.
pub struct Membership {
    pub outbound: mpsc::Receiver<Bytes>,
    pub control: watch::Receiver<ConnectionControl>,
}

pub fn encode_frame(message_serial_out: &MessageSerialOut) -> Bytes {
    Bytes::from(protocol::encode_message_out(message_serial_out))
}

pub fn create_membership(connection_id: ConnectionId) -> (ConnectionEndpoint, Membership) {
    let (outbound_sender, outbound_receiver): (mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) =
        mpsc::channel(MEMBERSHIP_CHANNEL_CAPACITY);
    let (control_sender, control_receiver): (watch::Sender<ConnectionControl>, watch::Receiver<ConnectionControl>) =
        watch::channel(ConnectionControl::Open);

    let endpoint: ConnectionEndpoint = ConnectionEndpoint {
        connection_id,
        outbound: outbound_sender,
        control: control_sender,
    };
    let membership: Membership = Membership {
        outbound: outbound_receiver,
        control: control_receiver,
    };

    (endpoint, membership)
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib socket::connection_model`

Expected: `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 27 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/socket/mod.rs server/src/socket/connection_model.rs server/src/game/mod.rs server/src/game/game_handle.rs
git commit -m "connection model and game handle"
```

### Task 11: Server registries

**Files:**
- Create: `server/src/registries/mod.rs`
- Create: `server/src/registries/registries.rs`
- Create: `server/src/game/game_registry.rs` (statics only)
- Create: `server/src/socket/connection_limit.rs` (statics only)
- Create: `server/src/lobby/mod.rs`
- Create: `server/src/lobby/lobby_broadcast.rs` (static only)
- Modify: `server/src/socket/connection_model.rs`
- Modify: `server/src/lib.rs`, `server/src/game/mod.rs`, `server/src/socket/mod.rs`
- Test: inline `mod tests` in `server/src/registries/registries.rs` and `server/src/socket/connection_model.rs`

Each registry static lives in its feature module (server.md, registries); `ServerRegistries` holds one `&'static` reference to each, and only it names them. This task creates the statics; Tasks 12, 13 and 22 add the code which uses them.

- [ ] **Step 1: Write the failing test**

Create `server/src/registries/registries.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn production_points_at_the_statics() {
        let registries: ServerRegistries = ServerRegistries::production();

        assert!(std::ptr::eq(registries.game_titles, &*game_registry::GAME_TITLES));
        assert!(std::ptr::eq(
            registries.connection_count,
            &connection_limit::CONNECTION_COUNT
        ));
        assert!(std::ptr::eq(
            registries.lobby_subscribers,
            &*lobby_broadcast::LOBBY_SUBSCRIBERS
        ));
    }

    #[test]
    fn leaked_gives_registries_which_share_nothing() {
        let first_registries: ServerRegistries = ServerRegistries::leaked();
        let second_registries: ServerRegistries = ServerRegistries::leaked();

        first_registries.game_titles.insert(String::from("Arena"), GameId(1));
        first_registries.game_count.fetch_add(1, Ordering::SeqCst);

        assert!(second_registries.game_titles.is_empty());
        assert_eq!(second_registries.game_count.load(Ordering::SeqCst), 0);
        assert_eq!(
            second_registries.next_game_id.load(Ordering::SeqCst),
            game_registry::FIRST_GAME_ID
        );
    }
}
```

Create `server/src/registries/mod.rs`:

```rust
pub mod registries;

pub use registries::*;
```

Replace the whole contents of `server/src/lib.rs` with:

```rust
//! The game server.

pub mod config;
pub mod game;
pub mod password;
pub mod registries;
pub mod socket;
```

In `server/src/socket/connection_model.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn allocate_connection_id_counts_up_from_the_first_connection_id() {
        let registries: ServerRegistries = ServerRegistries::leaked();

        assert_eq!(allocate_connection_id(registries), ConnectionId(FIRST_CONNECTION_ID));
        assert_eq!(
            allocate_connection_id(registries),
            ConnectionId(FIRST_CONNECTION_ID + 1)
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib -- registries::registries socket::connection_model`

Expected: compile failure, first line ``error[E0425]: cannot find type `ServerRegistries` in this scope``.

- [ ] **Step 3: Implement**

Create `server/src/game/game_registry.rs`:

```rust
use std::sync::LazyLock;
use std::sync::atomic::AtomicU32;

use dashmap::DashMap;

use shared::game::GameId;

use crate::game::GameHandle;

pub const FIRST_GAME_ID: u32 = 1;

pub static GAMES: LazyLock<DashMap<GameId, GameHandle>> = LazyLock::new(DashMap::new);
pub static GAME_TITLES: LazyLock<DashMap<String, GameId>> = LazyLock::new(DashMap::new);
pub static NEXT_GAME_ID: AtomicU32 = AtomicU32::new(FIRST_GAME_ID);
pub static GAME_COUNT: AtomicU32 = AtomicU32::new(0);
```

Create `server/src/socket/connection_limit.rs`:

```rust
use std::net::IpAddr;
use std::sync::LazyLock;
use std::sync::atomic::AtomicU32;

use dashmap::DashMap;

pub static CONNECTION_COUNT: AtomicU32 = AtomicU32::new(0);
pub static CONNECTIONS_PER_IP: LazyLock<DashMap<IpAddr, u32>> = LazyLock::new(DashMap::new);
```

Create `server/src/lobby/lobby_broadcast.rs`:

```rust
use std::sync::LazyLock;

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::mpsc;

use crate::socket::ConnectionId;

pub static LOBBY_SUBSCRIBERS: LazyLock<DashMap<ConnectionId, mpsc::Sender<Bytes>>> = LazyLock::new(DashMap::new);
```

Create `server/src/lobby/mod.rs`:

```rust
pub mod lobby_broadcast;

pub use lobby_broadcast::*;
```

Replace the whole contents of `server/src/game/mod.rs` with:

```rust
pub mod cursor_clamp;
pub mod game_handle;
pub mod game_registry;
pub mod game_runtime;

pub use cursor_clamp::*;
pub use game_handle::*;
pub use game_registry::*;
pub use game_runtime::*;
```

Replace the whole contents of `server/src/socket/mod.rs` with:

```rust
pub mod connection_limit;
pub mod connection_model;
pub mod rate_limit;

pub use connection_limit::*;
pub use connection_model::*;
pub use rate_limit::*;
```

Replace the whole contents of `server/src/lib.rs` with:

```rust
//! The game server.

pub mod config;
pub mod game;
pub mod lobby;
pub mod password;
pub mod registries;
pub mod socket;
```

In `server/src/registries/registries.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::net::IpAddr;
use std::sync::atomic::{AtomicU32, AtomicU64};

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::mpsc;

use shared::game::GameId;

use crate::game::GameHandle;
use crate::game::game_registry;
use crate::lobby::lobby_broadcast;
use crate::password::PasswordFailureWindow;
use crate::password::password_failure_limit;
use crate::socket::ConnectionId;
use crate::socket::{connection_limit, connection_model};

/// Every in-memory registry of one server.
#[derive(Clone, Copy)]
pub struct ServerRegistries {
    pub games: &'static DashMap<GameId, GameHandle>,
    pub game_titles: &'static DashMap<String, GameId>,
    pub next_game_id: &'static AtomicU32,
    pub game_count: &'static AtomicU32,
    pub next_connection_id: &'static AtomicU64,
    pub connection_count: &'static AtomicU32,
    pub connections_per_ip: &'static DashMap<IpAddr, u32>,
    pub lobby_subscribers: &'static DashMap<ConnectionId, mpsc::Sender<Bytes>>,
    pub password_failures_per_ip: &'static DashMap<IpAddr, PasswordFailureWindow>,
}

impl ServerRegistries {
    pub fn production() -> ServerRegistries {
        ServerRegistries {
            games: &game_registry::GAMES,
            game_titles: &game_registry::GAME_TITLES,
            next_game_id: &game_registry::NEXT_GAME_ID,
            game_count: &game_registry::GAME_COUNT,
            next_connection_id: &connection_model::NEXT_CONNECTION_ID,
            connection_count: &connection_limit::CONNECTION_COUNT,
            connections_per_ip: &connection_limit::CONNECTIONS_PER_IP,
            lobby_subscribers: &lobby_broadcast::LOBBY_SUBSCRIBERS,
            password_failures_per_ip: &password_failure_limit::PASSWORD_FAILURES_PER_IP,
        }
    }

    /// Fresh registries which are never freed, so parallel in-process servers share nothing.
    pub fn leaked() -> ServerRegistries {
        ServerRegistries {
            games: Box::leak(Box::new(DashMap::new())),
            game_titles: Box::leak(Box::new(DashMap::new())),
            next_game_id: Box::leak(Box::new(AtomicU32::new(game_registry::FIRST_GAME_ID))),
            game_count: Box::leak(Box::new(AtomicU32::new(0))),
            next_connection_id: Box::leak(Box::new(AtomicU64::new(connection_model::FIRST_CONNECTION_ID))),
            connection_count: Box::leak(Box::new(AtomicU32::new(0))),
            connections_per_ip: Box::leak(Box::new(DashMap::new())),
            lobby_subscribers: Box::leak(Box::new(DashMap::new())),
            password_failures_per_ip: Box::leak(Box::new(DashMap::new())),
        }
    }
}
```

In `server/src/socket/connection_model.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

use bytes::Bytes;
use tokio::sync::{mpsc, watch};

use shared::game::GameId;
use shared::member::MemberId;
use shared::protocol;
use shared::protocol::{CloseReasonKind, MessageSerialOut};

use crate::game::ConnectionEndpoint;
use crate::registries::ServerRegistries;

pub const FIRST_CONNECTION_ID: u64 = 1;
pub const MEMBERSHIP_CHANNEL_CAPACITY: usize = 256;
pub const NORMAL_CLOSURE_CODE: u16 = 1000;

pub static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(FIRST_CONNECTION_ID);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionPhase {
    Lobby,
    InGame { game_id: GameId, member_id: MemberId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionControl {
    Open,
    Close(CloseCauseKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolViolationKind {
    Decode,
    Conversion,
    TextFrame,
    Oversize,
    Validation,
}

impl ProtocolViolationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ProtocolViolationKind::Decode => "decode",
            ProtocolViolationKind::Conversion => "conversion",
            ProtocolViolationKind::TextFrame => "text frame",
            ProtocolViolationKind::Oversize => "oversize",
            ProtocolViolationKind::Validation => "validation",
        }
    }
}

/// Why the server closes a connection, with the cause of a protocol violation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseCauseKind {
    Reason(CloseReasonKind),
    ProtocolViolation(ProtocolViolationKind),
    /// Answers the client's own close.
    ClientClosed,
}

impl CloseCauseKind {
    pub fn close_code(self) -> u16 {
        match self {
            CloseCauseKind::Reason(close_reason) => close_reason.close_code(),
            CloseCauseKind::ProtocolViolation(_) => CloseReasonKind::ProtocolViolation.close_code(),
            CloseCauseKind::ClientClosed => NORMAL_CLOSURE_CODE,
        }
    }

    pub fn description(self) -> Option<String> {
        match self {
            CloseCauseKind::Reason(close_reason) => close_reason.reason_text(),
            CloseCauseKind::ProtocolViolation(violation) => Some(String::from(violation.as_str())),
            CloseCauseKind::ClientClosed => None,
        }
    }
}

/// The receiving ends of a membership, kept by the connection.
pub struct Membership {
    pub outbound: mpsc::Receiver<Bytes>,
    pub control: watch::Receiver<ConnectionControl>,
}

pub fn encode_frame(message_serial_out: &MessageSerialOut) -> Bytes {
    Bytes::from(protocol::encode_message_out(message_serial_out))
}

pub fn allocate_connection_id(registries: ServerRegistries) -> ConnectionId {
    ConnectionId(registries.next_connection_id.fetch_add(1, Ordering::SeqCst))
}

pub fn create_membership(connection_id: ConnectionId) -> (ConnectionEndpoint, Membership) {
    let (outbound_sender, outbound_receiver): (mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) =
        mpsc::channel(MEMBERSHIP_CHANNEL_CAPACITY);
    let (control_sender, control_receiver): (watch::Sender<ConnectionControl>, watch::Receiver<ConnectionControl>) =
        watch::channel(ConnectionControl::Open);

    let endpoint: ConnectionEndpoint = ConnectionEndpoint {
        connection_id,
        outbound: outbound_sender,
        control: control_sender,
    };
    let membership: Membership = Membership {
        outbound: outbound_receiver,
        control: control_receiver,
    };

    (endpoint, membership)
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib -- registries::registries socket::connection_model`

Expected: `test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 27 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/lib.rs server/src/registries/mod.rs server/src/registries/registries.rs server/src/game/mod.rs server/src/game/game_registry.rs server/src/socket/mod.rs server/src/socket/connection_limit.rs server/src/socket/connection_model.rs server/src/lobby/mod.rs server/src/lobby/lobby_broadcast.rs
git commit -m "server registries"
```

### Task 12: Connection limits

**Files:**
- Modify: `server/src/socket/connection_limit.rs`
- Test: inline `mod tests` in `server/src/socket/connection_limit.rs`

4096 connections per server, 16 per IP (protocol.md, limits), reserved atomically: `fetch_add`, then `fetch_sub` when the previous count was already at the limit; per IP through `entry()` under the shard lock (server.md, registries).

- [ ] **Step 1: Write the failing test**

Append to the end of `server/src/socket/connection_limit.rs`, after one blank line:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7));
    const OTHER_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 8));

    #[test]
    fn reserve_connection_rejects_beyond_the_per_ip_limit_only_for_that_ip() {
        let registries: ServerRegistries = ServerRegistries::leaked();

        for _ in 0..protocol_limits::CONNECTION_LIMIT_PER_IP {
            reserve_connection(registries, IP).unwrap();
        }

        assert_eq!(reserve_connection(registries, IP), Err(ConnectionLimitReached));
        assert_eq!(reserve_connection(registries, OTHER_IP), Ok(()));
        assert_eq!(
            registries.connection_count.load(Ordering::SeqCst),
            protocol_limits::CONNECTION_LIMIT_PER_IP + 1,
        );
    }

    #[test]
    fn reserve_connection_rejects_beyond_the_server_limit_without_counting_the_ip() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        registries.connection_count.store(protocol_limits::CONNECTION_LIMIT_PER_SERVER, Ordering::SeqCst);

        assert_eq!(reserve_connection(registries, IP), Err(ConnectionLimitReached));
        assert_eq!(
            registries.connection_count.load(Ordering::SeqCst),
            protocol_limits::CONNECTION_LIMIT_PER_SERVER,
        );
        assert!(registries.connections_per_ip.is_empty());
    }

    #[test]
    fn release_connection_frees_the_slots_and_forgets_an_ip_without_connections() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        reserve_connection(registries, IP).unwrap();
        reserve_connection(registries, IP).unwrap();

        release_connection(registries, IP);

        assert_eq!(*registries.connections_per_ip.get(&IP).unwrap(), 1);

        release_connection(registries, IP);

        assert!(registries.connections_per_ip.is_empty());
        assert_eq!(registries.connection_count.load(Ordering::SeqCst), 0);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib socket::connection_limit`

Expected: compile failure, first line ``error[E0425]: cannot find type `ServerRegistries` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/socket/connection_limit.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::net::IpAddr;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU32, Ordering};

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;

use shared::protocol::protocol_limits;

use crate::registries::ServerRegistries;

pub static CONNECTION_COUNT: AtomicU32 = AtomicU32::new(0);
pub static CONNECTIONS_PER_IP: LazyLock<DashMap<IpAddr, u32>> = LazyLock::new(DashMap::new);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConnectionLimitReached;

/// Counts the connection against the server and its IP; nothing stays reserved on `Err`.
pub fn reserve_connection(registries: ServerRegistries, ip: IpAddr) -> Result<(), ConnectionLimitReached> {
    let previous_connection_count: u32 = registries.connection_count.fetch_add(1, Ordering::SeqCst);

    if previous_connection_count >= protocol_limits::CONNECTION_LIMIT_PER_SERVER {
        registries.connection_count.fetch_sub(1, Ordering::SeqCst);

        return Err(ConnectionLimitReached);
    }

    let ip_reservation: Result<(), ConnectionLimitReached> = reserve_ip_connection(registries, ip);

    if ip_reservation.is_err() {
        registries.connection_count.fetch_sub(1, Ordering::SeqCst);
    }

    ip_reservation
}

pub fn release_connection(registries: ServerRegistries, ip: IpAddr) {
    registries.connection_count.fetch_sub(1, Ordering::SeqCst);

    let Entry::Occupied(mut occupied_entry) = registries.connections_per_ip.entry(ip) else {
        return;
    };

    if *occupied_entry.get() <= 1 {
        occupied_entry.remove();
    } else {
        *occupied_entry.get_mut() -= 1;
    }
}

fn reserve_ip_connection(registries: ServerRegistries, ip: IpAddr) -> Result<(), ConnectionLimitReached> {
    match registries.connections_per_ip.entry(ip) {
        Entry::Occupied(mut occupied_entry) => {
            if *occupied_entry.get() >= protocol_limits::CONNECTION_LIMIT_PER_IP {
                return Err(ConnectionLimitReached);
            }

            *occupied_entry.get_mut() += 1;

            Ok(())
        }
        Entry::Vacant(vacant_entry) => {
            vacant_entry.insert(1);

            Ok(())
        }
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib socket::connection_limit`

Expected: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 34 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/socket/connection_limit.rs
git commit -m "connection limits"
```

### Task 13: Game registry

**Files:**
- Modify: `server/src/game/game_registry.rs`
- Create: `server/src/game/test_fixture.rs`
- Modify: `server/src/game/mod.rs`
- Test: inline `mod tests` in `server/src/game/game_registry.rs`

Creation reserves the title through `entry()`, then a game slot; a failure releases both (server.md, lifecycle). Removal releases the title only while it still names this game, so a newer game which reused the title keeps it. `test_fixture.rs` holds the unit-test fixtures of this and later game, lobby and site tests.

- [ ] **Step 1: Write the failing test**

Create `server/src/game/test_fixture.rs`:

```rust
use std::path::PathBuf;

use tokio::sync::{mpsc, watch};

use shared::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use shared::game::{GameId, GameModeKind, GameSettings, GameState, GameSummary};
use shared::member::{Appearance, Joiner, OrganismColorKind, SkinKind, TeamChoiceKind};
use shared::world::WorldShapeKind;

use crate::game::{GameHandle, GameInput, GameRequest};

pub const FIXTURE_SEED: u64 = 0x5eed;

/// Square 800 px rectangle; survival has a player minimum of 2 and skirmish two teams.
pub fn create_settings(mode: GameModeKind, player_cap: u8) -> GameSettings {
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
        world_shape: WorldShapeKind::Rectangle,
        world_width_pixels: 800,
        world_height_pixels: 800,
        player_minimum,
        player_cap,
        team_count,
        leaderboard_length: 10,
    }
}

pub fn create_state(mode: GameModeKind, player_cap: u8) -> GameState {
    GameState::new(create_settings(mode, player_cap), FIXTURE_SEED)
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

pub fn create_joiner(screen_name: &str, team: TeamChoiceKind) -> Joiner {
    Joiner {
        screen_name: String::from(screen_name),
        loadout: create_loadout(),
        team,
    }
}

/// Unconnected channels; the receivers are dropped.
pub fn create_game_handle(game_id: GameId, settings: GameSettings) -> GameHandle {
    let (request_sender, _): (mpsc::Sender<GameRequest>, mpsc::Receiver<GameRequest>) = mpsc::channel(1);
    let (input_sender, _): (mpsc::Sender<GameInput>, mpsc::Receiver<GameInput>) = mpsc::channel(1);
    let state: GameState = GameState::new(settings.clone(), FIXTURE_SEED);
    let (_, summary_receiver): (watch::Sender<GameSummary>, watch::Receiver<GameSummary>) =
        watch::channel(GameSummary::from_game_state(game_id, &state, false));

    GameHandle {
        requests: request_sender,
        inputs: input_sender,
        summary: summary_receiver,
        password_hash: None,
        settings,
    }
}

/// Empty, and unique to the test process and `name`.
pub fn create_temporary_directory(name: &str) -> PathBuf {
    let directory: PathBuf = std::env::temp_dir().join(format!("bacter-server-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the temporary directory is created");

    directory
}
```

Replace the whole contents of `server/src/game/mod.rs` with:

```rust
pub mod cursor_clamp;
pub mod game_handle;
pub mod game_registry;
pub mod game_runtime;
// Fixtures shared by the unit tests of several modules; absent from every non-test build.
#[cfg(test)]
pub mod test_fixture;

pub use cursor_clamp::*;
pub use game_handle::*;
pub use game_registry::*;
pub use game_runtime::*;
```

Append to the end of `server/src/game/game_registry.rs`, after one blank line:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::GameModeKind;

    #[test]
    fn reserve_game_allocates_increasing_game_ids() {
        let registries: ServerRegistries = ServerRegistries::leaked();

        assert_eq!(
            reserve_game(registries, "First").unwrap().game_id,
            GameId(FIRST_GAME_ID)
        );
        assert_eq!(
            reserve_game(registries, "Second").unwrap().game_id,
            GameId(FIRST_GAME_ID + 1)
        );
        assert_eq!(registries.game_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn reserve_game_rejects_a_taken_title_and_keeps_no_slot() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        reserve_game(registries, "Arena").unwrap();

        assert_eq!(reserve_game(registries, "Arena"), Err(RejectionKind::TitleTaken));
        assert_eq!(registries.game_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn reserve_game_rejects_beyond_the_game_limit_and_releases_the_title() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        registries.game_count.store(protocol_limits::GAME_LIMIT_PER_SERVER, Ordering::SeqCst);

        assert_eq!(reserve_game(registries, "Arena"), Err(RejectionKind::ServerFull));
        assert_eq!(
            registries.game_count.load(Ordering::SeqCst),
            protocol_limits::GAME_LIMIT_PER_SERVER
        );
        assert!(registries.game_titles.is_empty());
    }

    #[test]
    fn cancel_reservation_releases_the_title_and_the_slot() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let reservation: GameReservation = reserve_game(registries, "Arena").unwrap();

        cancel_reservation(registries, &reservation);

        assert!(registries.game_titles.is_empty());
        assert_eq!(registries.game_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn get_game_clones_a_registered_handle() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let reservation: GameReservation = reserve_game(registries, "Arena").unwrap();
        insert_game(
            registries,
            reservation.game_id,
            test_fixture::create_game_handle(
                reservation.game_id,
                test_fixture::create_settings(GameModeKind::FreeForAll, 8),
            ),
        );

        assert!(get_game(registries, reservation.game_id).is_some());
        assert!(get_game(registries, GameId(99)).is_none());
    }

    #[test]
    fn remove_game_keeps_a_newer_reservation_of_the_title() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let first_reservation: GameReservation = reserve_game(registries, "Arena").unwrap();
        registries.game_titles.insert(String::from("Arena"), GameId(42));

        remove_game(registries, first_reservation.game_id, "Arena");

        assert_eq!(*registries.game_titles.get("Arena").unwrap(), GameId(42));
        assert_eq!(registries.game_count.load(Ordering::SeqCst), 0);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib game::game_registry`

Expected: compile failure, first line ``error[E0425]: cannot find type `ServerRegistries` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/game/game_registry.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU32, Ordering};

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;

use shared::game::GameId;
use shared::protocol::{RejectionKind, protocol_limits};

use crate::game::GameHandle;
use crate::registries::ServerRegistries;

pub const FIRST_GAME_ID: u32 = 1;

pub static GAMES: LazyLock<DashMap<GameId, GameHandle>> = LazyLock::new(DashMap::new);
pub static GAME_TITLES: LazyLock<DashMap<String, GameId>> = LazyLock::new(DashMap::new);
pub static NEXT_GAME_ID: AtomicU32 = AtomicU32::new(FIRST_GAME_ID);
pub static GAME_COUNT: AtomicU32 = AtomicU32::new(0);

/// A title and a game slot held for a game not yet registered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameReservation {
    pub game_id: GameId,
    pub title: String,
}

/// The title first, then a game slot; nothing stays reserved on `Err`.
pub fn reserve_game(registries: ServerRegistries, title: &str) -> Result<GameReservation, RejectionKind> {
    let game_id: GameId = GameId(registries.next_game_id.fetch_add(1, Ordering::SeqCst));

    reserve_title(registries, title, game_id)?;

    let slot_reservation: Result<(), RejectionKind> = reserve_game_slot(registries);

    if let Err(rejection) = slot_reservation {
        release_title(registries, title, game_id);

        return Err(rejection);
    }

    Ok(GameReservation {
        game_id,
        title: String::from(title),
    })
}

pub fn cancel_reservation(registries: ServerRegistries, reservation: &GameReservation) {
    release_title(registries, &reservation.title, reservation.game_id);
    registries.game_count.fetch_sub(1, Ordering::SeqCst);
}

pub fn insert_game(registries: ServerRegistries, game_id: GameId, game_handle: GameHandle) {
    registries.games.insert(game_id, game_handle);
}

/// A clone, so no map reference is held across an await.
pub fn get_game(registries: ServerRegistries, game_id: GameId) -> Option<GameHandle> {
    registries.games.get(&game_id).map(|entry| entry.value().clone())
}

/// A newer game holding the same title keeps it.
pub fn remove_game(registries: ServerRegistries, game_id: GameId, title: &str) {
    registries.games.remove(&game_id);
    release_title(registries, title, game_id);
    registries.game_count.fetch_sub(1, Ordering::SeqCst);
}

fn reserve_title(registries: ServerRegistries, title: &str, game_id: GameId) -> Result<(), RejectionKind> {
    match registries.game_titles.entry(String::from(title)) {
        Entry::Occupied(_) => Err(RejectionKind::TitleTaken),
        Entry::Vacant(vacant_entry) => {
            vacant_entry.insert(game_id);

            Ok(())
        }
    }
}

fn release_title(registries: ServerRegistries, title: &str, game_id: GameId) {
    registries.game_titles.remove_if(title, |_, reserved_game_id| *reserved_game_id == game_id);
}

fn reserve_game_slot(registries: ServerRegistries) -> Result<(), RejectionKind> {
    let previous_game_count: u32 = registries.game_count.fetch_add(1, Ordering::SeqCst);

    if previous_game_count >= protocol_limits::GAME_LIMIT_PER_SERVER {
        registries.game_count.fetch_sub(1, Ordering::SeqCst);

        return Err(RejectionKind::ServerFull);
    }

    Ok(())
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib game::game_registry`

Expected: `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/game/mod.rs server/src/game/game_registry.rs server/src/game/test_fixture.rs
git commit -m "game registry"
```

### Task 14: Pending admissions

**Files:**
- Create: `server/src/game/pending_admissions.rs`
- Modify: `server/src/game/mod.rs`
- Test: inline `mod tests` in `server/src/game/pending_admissions.rs`

What the requests admitted since the last bundle add to the state: screen names, members, pure Spectators, spawns and their teams (server.md, lifecycle, admission). Every admission check reads the state plus this ledger; the next member id is `state.next_member_id + pending joins`.

- [ ] **Step 1: Write the failing test**

Create `server/src/game/pending_admissions.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::GameModeKind;
    use shared::geometry::WorldPoint;
    use shared::member::{Member, Score};
    use shared::organism::Organism;

    fn create_member(member_id: u32, role: MemberRoleKind, team: Option<TeamKind>, is_alive: bool) -> Member {
        Member {
            member_id: MemberId(member_id),
            screen_name: format!("member {member_id}"),
            role,
            loadout: Some(test_fixture::create_loadout()),
            team,
            score: Score::zero(),
            organism: is_alive.then(|| Organism::new(WorldPoint { x: 100, y: 100 })),
        }
    }

    fn create_state_with_members(mode: GameModeKind, members: Vec<Member>) -> GameState {
        let mut state: GameState = test_fixture::create_state(mode, 8);

        for member in members {
            state.next_member_id = member.member_id.next();
            state.members.insert(member.member_id, member);
        }

        state
    }

    #[test]
    fn is_screen_name_taken_checks_members_and_pending_names() {
        let state: GameState = create_state_with_members(
            GameModeKind::FreeForAll,
            vec![create_member(0, MemberRoleKind::Participant, None, true)],
        );
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();
        pending_admissions.record_member("Blob", MemberRoleKind::Participant);

        assert!(pending_admissions.is_screen_name_taken(&state, "member 0"));
        assert!(pending_admissions.is_screen_name_taken(&state, "Blob"));
        assert!(!pending_admissions.is_screen_name_taken(&state, "blob"));
    }

    #[test]
    fn get_member_count_and_the_other_counts_add_pending_admissions_to_the_state() {
        let state: GameState = create_state_with_members(
            GameModeKind::FreeForAll,
            vec![
                create_member(0, MemberRoleKind::Participant, None, true),
                create_member(1, MemberRoleKind::Participant, None, false),
                create_member(2, MemberRoleKind::Spectator, None, false),
            ],
        );
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();
        pending_admissions.record_member("Watcher", MemberRoleKind::Spectator);
        pending_admissions.record_member("Late", MemberRoleKind::Participant);
        pending_admissions.record_spawn(MemberId(4), None);
        pending_admissions.record_spawn(MemberId(1), None);

        assert_eq!(pending_admissions.get_member_count(&state), 5);
        assert_eq!(pending_admissions.get_spectator_count(&state), 2);
        assert_eq!(pending_admissions.get_spawning_count(&state), 3);
        assert_eq!(pending_admissions.get_next_member_id(&state), MemberId(5));
        assert!(pending_admissions.is_spawn_pending(MemberId(1)));
        assert!(!pending_admissions.is_spawn_pending(MemberId(0)));
    }

    #[test]
    fn get_team_sizes_adds_pending_teams_and_skips_the_excluded_member() {
        let state: GameState = create_state_with_members(
            GameModeKind::Skirmish,
            vec![
                create_member(0, MemberRoleKind::Participant, Some(TeamKind::Red), true),
                create_member(1, MemberRoleKind::Participant, Some(TeamKind::Red), false),
            ],
        );
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();
        pending_admissions.record_spawn(MemberId(2), Some(TeamKind::Blue));

        let team_sizes: TeamSizes = pending_admissions.get_team_sizes(&state, 2, Some(MemberId(1)));

        assert_eq!(team_sizes.get_size(TeamKind::Red), Some(1));
        assert_eq!(team_sizes.get_size(TeamKind::Blue), Some(1));
    }

    #[test]
    fn clear_forgets_every_pending_admission() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();
        pending_admissions.record_member("Blob", MemberRoleKind::Participant);
        pending_admissions.record_spawn(MemberId(0), None);

        pending_admissions.clear();

        assert_eq!(pending_admissions, PendingAdmissions::new());
        assert_eq!(pending_admissions.get_next_member_id(&state), MemberId(0));
    }
}
```

Replace the whole contents of `server/src/game/mod.rs` with:

```rust
pub mod cursor_clamp;
pub mod game_handle;
pub mod game_registry;
pub mod game_runtime;
pub mod pending_admissions;
// Fixtures shared by the unit tests of several modules; absent from every non-test build.
#[cfg(test)]
pub mod test_fixture;

pub use cursor_clamp::*;
pub use game_handle::*;
pub use game_registry::*;
pub use game_runtime::*;
pub use pending_admissions::*;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib game::pending_admissions`

Expected: compile failure, first line ``error[E0425]: cannot find type `MemberRoleKind` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/game/pending_admissions.rs`, replace everything above `#[cfg(test)]` with:

```rust
use shared::game::GameState;
use shared::member::{MemberId, MemberRoleKind, TeamKind, TeamSizes};

/// What the requests admitted since the last bundle add to the state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingAdmissions {
    screen_names: Vec<String>,
    member_count: u32,
    spectator_count: u32,
    spawning_member_ids: Vec<MemberId>,
    /// Of the pending spawns in skirmish.
    team_additions: Vec<TeamKind>,
}

impl PendingAdmissions {
    pub fn new() -> PendingAdmissions {
        PendingAdmissions::default()
    }

    pub fn clear(&mut self) {
        *self = PendingAdmissions::new();
    }

    pub fn is_screen_name_taken(&self, state: &GameState, screen_name: &str) -> bool {
        let is_member_name: bool = state.members.values().any(|member| member.screen_name == screen_name);

        is_member_name || self.screen_names.iter().any(|pending_name| pending_name == screen_name)
    }

    pub fn get_member_count(&self, state: &GameState) -> u32 {
        get_count(state.members.len()) + self.member_count
    }

    /// Pure Spectators.
    pub fn get_spectator_count(&self, state: &GameState) -> u32 {
        let spectator_count: usize =
            state.members.values().filter(|member| member.role == MemberRoleKind::Spectator).count();

        get_count(spectator_count) + self.spectator_count
    }

    /// Alive organisms plus pending spawns.
    pub fn get_spawning_count(&self, state: &GameState) -> u32 {
        state.alive_organism_count() + get_count(self.spawning_member_ids.len())
    }

    pub fn is_spawn_pending(&self, member_id: MemberId) -> bool {
        self.spawning_member_ids.contains(&member_id)
    }

    /// `excluded_member_id` is not counted.
    pub fn get_team_sizes(&self, state: &GameState, team_count: u8, excluded_member_id: Option<MemberId>) -> TeamSizes {
        let mut team_sizes: TeamSizes = TeamSizes::from_members(&state.members, team_count, excluded_member_id);

        for team in &self.team_additions {
            team_sizes.add_member(*team);
        }

        team_sizes
    }

    pub fn get_next_member_id(&self, state: &GameState) -> MemberId {
        MemberId(state.next_member_id.0 + self.member_count)
    }

    pub fn record_member(&mut self, screen_name: &str, role: MemberRoleKind) {
        self.screen_names.push(String::from(screen_name));
        self.member_count += 1;

        if role == MemberRoleKind::Spectator {
            self.spectator_count += 1;
        }
    }

    pub fn record_spawn(&mut self, member_id: MemberId, team: Option<TeamKind>) {
        self.spawning_member_ids.push(member_id);
        self.team_additions.extend(team);
    }
}

fn get_count(length: usize) -> u32 {
    u32::try_from(length).unwrap_or(u32::MAX)
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib game::pending_admissions`

Expected: `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 43 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/game/mod.rs server/src/game/pending_admissions.rs
git commit -m "pending admissions"
```

### Task 15: Join, spectate and respawn admission

**Files:**
- Create: `server/src/game/membership.rs`
- Modify: `server/src/game/mod.rs`
- Test: inline `mod tests` in `server/src/game/membership.rs`

server.md (lifecycle), in its check order. Join: name, member limit (`player_cap + 64`), spawn slot (alive organisms plus pending spawns below the cap; skipped while a survival round is Playing or PostRound, when the joiner waits without `SpawnRequested`), team balance. Spectate: name, 64 pure Spectators, member limit. Respawn: `NotDead`, `GameFull`, `RoundInProgress`, `TeamUnbalanced`. With these checks `step` never declines a spawn the server admitted, except for `PositionNotFound`.

- [ ] **Step 1: Write the failing test**

Create `server/src/game/membership.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::{GameModeKind, Tick};
    use shared::geometry::WorldPoint;
    use shared::member::Score;
    use shared::organism::Organism;
    use shared::round::RoundState;

    fn add_participant(state: &mut GameState, screen_name: &str, team: Option<TeamKind>, is_alive: bool) -> MemberId {
        let member_id: MemberId = state.next_member_id;
        state.members.insert(
            member_id,
            Member {
                member_id,
                screen_name: String::from(screen_name),
                role: MemberRoleKind::Participant,
                loadout: Some(test_fixture::create_loadout()),
                team,
                score: Score::zero(),
                organism: is_alive.then(|| Organism::new(WorldPoint { x: 100, y: 100 })),
            },
        );
        state.next_member_id = member_id.next();

        member_id
    }

    fn add_spectator(state: &mut GameState, screen_name: &str) -> MemberId {
        let member_id: MemberId = add_participant(state, screen_name, None, false);
        let member: &mut Member = state.members.get_mut(&member_id).unwrap();
        member.role = MemberRoleKind::Spectator;
        member.loadout = None;

        member_id
    }

    fn set_round_phase(state: &mut GameState, phase: RoundPhase) {
        state.round = Some(RoundState {
            phase,
            phase_started_at: Tick(0),
        });
    }

    #[test]
    fn admit_join_allocates_the_next_member_id_and_spawns_the_joiner() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        add_participant(&mut state, "First", None, true);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();
        let joiner: Joiner = test_fixture::create_joiner("Blob", TeamChoiceKind::Auto);

        let admission: JoinAdmission = admit_join(&state, &mut pending_admissions, &joiner).unwrap();

        assert_eq!(
            admission,
            JoinAdmission {
                member_id: MemberId(1),
                member_events: vec![
                    MemberEvent::Joined {
                        member_id: MemberId(1),
                        screen_name: String::from("Blob"),
                        role: MemberRoleKind::Participant,
                        loadout: Some(test_fixture::create_loadout()),
                        team: None,
                    },
                    MemberEvent::SpawnRequested {
                        member_id: MemberId(1),
                        loadout: test_fixture::create_loadout(),
                        team: None,
                    },
                ],
            },
        );
        assert_eq!(pending_admissions.get_next_member_id(&state), MemberId(2));
    }

    #[test]
    fn admit_join_counts_pending_joins_for_names_slots_and_member_ids() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 2);
        add_participant(&mut state, "First", None, true);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();

        let first_admission: JoinAdmission = admit_join(
            &state,
            &mut pending_admissions,
            &test_fixture::create_joiner("Blob", TeamChoiceKind::Auto),
        )
        .unwrap();

        assert_eq!(first_admission.member_id, MemberId(1));
        assert_eq!(
            admit_join(
                &state,
                &mut pending_admissions,
                &test_fixture::create_joiner("Blob", TeamChoiceKind::Auto),
            ),
            Err(RejectionKind::ScreenNameTaken),
        );
        assert_eq!(
            admit_join(
                &state,
                &mut pending_admissions,
                &test_fixture::create_joiner("Other", TeamChoiceKind::Auto),
            ),
            Err(RejectionKind::GameFull),
        );
    }

    #[test]
    fn admit_join_rejects_a_name_taken_by_a_member() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        add_participant(&mut state, "Blob", None, false);

        assert_eq!(
            admit_join(
                &state,
                &mut PendingAdmissions::new(),
                &test_fixture::create_joiner("Blob", TeamChoiceKind::Auto),
            ),
            Err(RejectionKind::ScreenNameTaken),
        );
    }

    #[test]
    fn admit_join_rejects_at_the_member_limit() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 2);

        for spectator_index in 0..protocol_limits::get_member_limit(2) {
            add_spectator(&mut state, &format!("Watcher {spectator_index}"));
        }

        assert_eq!(
            admit_join(
                &state,
                &mut PendingAdmissions::new(),
                &test_fixture::create_joiner("Blob", TeamChoiceKind::Auto),
            ),
            Err(RejectionKind::GameFull),
        );
    }

    #[test]
    fn admit_join_admits_without_spawning_while_a_survival_round_is_in_progress() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Survival, 2);
        add_participant(&mut state, "First", None, true);
        add_participant(&mut state, "Second", None, true);
        set_round_phase(&mut state, RoundPhase::Playing);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();

        let admission: JoinAdmission = admit_join(
            &state,
            &mut pending_admissions,
            &test_fixture::create_joiner("Late", TeamChoiceKind::Auto),
        )
        .unwrap();

        assert_eq!(admission.member_events.len(), 1);
        assert!(!pending_admissions.is_spawn_pending(admission.member_id));
    }

    #[test]
    fn admit_join_assigns_auto_to_the_smallest_team_counting_pending_joins() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, 8);
        add_participant(&mut state, "First", Some(TeamKind::Red), true);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();

        let blue_admission: JoinAdmission = admit_join(
            &state,
            &mut pending_admissions,
            &test_fixture::create_joiner("Second", TeamChoiceKind::Auto),
        )
        .unwrap();
        let red_admission: JoinAdmission = admit_join(
            &state,
            &mut pending_admissions,
            &test_fixture::create_joiner("Third", TeamChoiceKind::Auto),
        )
        .unwrap();

        assert!(matches!(
            blue_admission.member_events[0],
            MemberEvent::Joined {
                team: Some(TeamKind::Blue),
                ..
            },
        ));
        assert!(matches!(
            red_admission.member_events[0],
            MemberEvent::Joined {
                team: Some(TeamKind::Red),
                ..
            },
        ));
    }

    #[test]
    fn admit_join_rejects_a_team_with_more_players_than_another() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, 8);
        add_participant(&mut state, "First", Some(TeamKind::Red), true);

        assert_eq!(
            admit_join(
                &state,
                &mut PendingAdmissions::new(),
                &test_fixture::create_joiner("Second", TeamChoiceKind::Team(TeamKind::Red)),
            ),
            Err(RejectionKind::TeamUnbalanced {
                requested: TeamKind::Red,
                smaller: TeamKind::Blue,
            }),
        );
    }

    #[test]
    fn admit_join_rejects_a_team_outside_the_game_naming_the_smallest_team() {
        let state: GameState = test_fixture::create_state(GameModeKind::Skirmish, 8);

        assert_eq!(
            admit_join(
                &state,
                &mut PendingAdmissions::new(),
                &test_fixture::create_joiner("First", TeamChoiceKind::Team(TeamKind::Pink)),
            ),
            Err(RejectionKind::TeamUnbalanced {
                requested: TeamKind::Pink,
                smaller: TeamKind::Red,
            }),
        );
    }

    #[test]
    fn admit_join_ignores_the_team_outside_skirmish() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);

        let admission: JoinAdmission = admit_join(
            &state,
            &mut PendingAdmissions::new(),
            &test_fixture::create_joiner("First", TeamChoiceKind::Team(TeamKind::Pink)),
        )
        .unwrap();

        assert!(matches!(
            admission.member_events[0],
            MemberEvent::Joined { team: None, .. }
        ));
    }

    #[test]
    fn admit_spectate_adds_a_spectator_without_loadout_or_team() {
        let state: GameState = test_fixture::create_state(GameModeKind::Skirmish, 8);

        assert_eq!(
            admit_spectate(&state, &mut PendingAdmissions::new(), "Watcher"),
            Ok(JoinAdmission {
                member_id: MemberId(0),
                member_events: vec![MemberEvent::Joined {
                    member_id: MemberId(0),
                    screen_name: String::from("Watcher"),
                    role: MemberRoleKind::Spectator,
                    loadout: None,
                    team: None,
                }],
            }),
        );
    }

    #[test]
    fn admit_spectate_rejects_at_the_spectator_limit_counting_pending_spectators() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();

        for spectator_index in 0..protocol_limits::SPECTATOR_LIMIT_PER_GAME - 1 {
            add_spectator(&mut state, &format!("Watcher {spectator_index}"));
        }

        admit_spectate(&state, &mut pending_admissions, "Last").unwrap();

        assert_eq!(
            admit_spectate(&state, &mut pending_admissions, "Extra"),
            Err(RejectionKind::SpectatorLimitReached),
        );
        assert_eq!(
            admit_spectate(&state, &mut pending_admissions, "Last"),
            Err(RejectionKind::ScreenNameTaken),
        );
    }

    #[test]
    fn admit_respawn_requests_a_spawn_for_a_dead_participant() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let member_id: MemberId = add_participant(&mut state, "First", None, false);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();

        assert_eq!(
            admit_respawn(
                &state,
                &mut pending_admissions,
                member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Auto,
            ),
            Ok(MemberEvent::SpawnRequested {
                member_id,
                loadout: test_fixture::create_loadout(),
                team: None,
            }),
        );
        assert_eq!(
            admit_respawn(
                &state,
                &mut pending_admissions,
                member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Auto,
            ),
            Err(RejectionKind::NotDead),
        );
    }

    #[test]
    fn admit_respawn_rejects_an_alive_or_unknown_member() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let alive_member_id: MemberId = add_participant(&mut state, "First", None, true);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();
        state.next_member_id = MemberId(5);

        for (member_id, rejection) in [
            (alive_member_id, RejectionKind::NotDead),
            (MemberId(3), RejectionKind::NotInGame),
            (MemberId(5), RejectionKind::NotDead),
        ] {
            assert_eq!(
                admit_respawn(
                    &state,
                    &mut pending_admissions,
                    member_id,
                    test_fixture::create_loadout(),
                    TeamChoiceKind::Auto,
                ),
                Err(rejection),
            );
        }
    }

    #[test]
    fn admit_respawn_rejects_at_the_player_cap_and_during_a_survival_round() {
        let mut full_state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 2);
        add_participant(&mut full_state, "First", None, true);
        add_participant(&mut full_state, "Second", None, true);
        let full_member_id: MemberId = add_participant(&mut full_state, "Third", None, false);
        let mut survival_state: GameState = test_fixture::create_state(GameModeKind::Survival, 8);
        let survival_member_id: MemberId = add_participant(&mut survival_state, "First", None, false);
        set_round_phase(&mut survival_state, RoundPhase::PostRound);

        assert_eq!(
            admit_respawn(
                &full_state,
                &mut PendingAdmissions::new(),
                full_member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Auto,
            ),
            Err(RejectionKind::GameFull),
        );
        assert_eq!(
            admit_respawn(
                &survival_state,
                &mut PendingAdmissions::new(),
                survival_member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Auto,
            ),
            Err(RejectionKind::RoundInProgress),
        );
    }

    #[test]
    fn admit_respawn_lets_a_member_stay_on_its_team_and_excludes_it_from_the_count() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::Skirmish, 8);
        add_participant(&mut state, "First", Some(TeamKind::Red), true);
        add_participant(&mut state, "Second", Some(TeamKind::Red), true);
        let member_id: MemberId = add_participant(&mut state, "Third", Some(TeamKind::Blue), false);
        let mut pending_admissions: PendingAdmissions = PendingAdmissions::new();

        assert_eq!(
            admit_respawn(
                &state,
                &mut pending_admissions,
                member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Team(TeamKind::Red),
            ),
            Err(RejectionKind::TeamUnbalanced {
                requested: TeamKind::Red,
                smaller: TeamKind::Blue,
            }),
        );
        assert_eq!(
            admit_respawn(
                &state,
                &mut pending_admissions,
                member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Team(TeamKind::Blue),
            ),
            Ok(MemberEvent::SpawnRequested {
                member_id,
                loadout: test_fixture::create_loadout(),
                team: Some(TeamKind::Blue),
            }),
        );
    }

    #[test]
    fn admit_respawn_converts_a_spectator() {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let member_id: MemberId = add_spectator(&mut state, "Watcher");

        assert!(
            admit_respawn(
                &state,
                &mut PendingAdmissions::new(),
                member_id,
                test_fixture::create_loadout(),
                TeamChoiceKind::Auto,
            )
            .is_ok()
        );
    }
}
```

Replace the whole contents of `server/src/game/mod.rs` with:

```rust
pub mod cursor_clamp;
pub mod game_handle;
pub mod game_registry;
pub mod game_runtime;
pub mod membership;
pub mod pending_admissions;
// Fixtures shared by the unit tests of several modules; absent from every non-test build.
#[cfg(test)]
pub mod test_fixture;

pub use cursor_clamp::*;
pub use game_handle::*;
pub use game_registry::*;
pub use game_runtime::*;
pub use membership::*;
pub use pending_admissions::*;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib game::membership`

Expected: compile failure, first line ``error[E0425]: cannot find type `GameState` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/game/membership.rs`, replace everything above `#[cfg(test)]` with:

```rust
use shared::ability::Loadout;
use shared::game::{GameState, MemberEvent};
use shared::member::team_assignment;
use shared::member::{
    Joiner, Member, MemberId, MemberRoleKind, TeamChoiceKind, TeamChoiceRejectionKind, TeamKind, TeamSizes,
};
use shared::protocol::{RejectionKind, protocol_limits};
use shared::round::RoundPhase;

use crate::game::PendingAdmissions;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinAdmission {
    pub member_id: MemberId,
    pub member_events: Vec<MemberEvent>,
}

/// A survival join while a round is in progress admits the joiner without spawning it.
pub fn admit_join(
    state: &GameState,
    pending_admissions: &mut PendingAdmissions,
    joiner: &Joiner,
) -> Result<JoinAdmission, RejectionKind> {
    check_new_member(state, pending_admissions, &joiner.screen_name)?;

    let member_id: MemberId = pending_admissions.get_next_member_id(state);
    let spawns: bool = !is_round_in_progress(state);

    if spawns {
        check_spawn_slot(state, pending_admissions)?;
    }

    let team: Option<TeamKind> = resolve_team(state, pending_admissions, joiner.team, None, None)?;
    let mut member_events: Vec<MemberEvent> = vec![MemberEvent::Joined {
        member_id,
        screen_name: joiner.screen_name.clone(),
        role: MemberRoleKind::Participant,
        loadout: Some(joiner.loadout),
        team,
    }];

    pending_admissions.record_member(&joiner.screen_name, MemberRoleKind::Participant);

    if spawns {
        member_events.push(MemberEvent::SpawnRequested {
            member_id,
            loadout: joiner.loadout,
            team,
        });
        pending_admissions.record_spawn(member_id, team);
    }

    Ok(JoinAdmission {
        member_id,
        member_events,
    })
}

pub fn admit_spectate(
    state: &GameState,
    pending_admissions: &mut PendingAdmissions,
    screen_name: &str,
) -> Result<JoinAdmission, RejectionKind> {
    if pending_admissions.is_screen_name_taken(state, screen_name) {
        return Err(RejectionKind::ScreenNameTaken);
    }

    if pending_admissions.get_spectator_count(state) >= protocol_limits::SPECTATOR_LIMIT_PER_GAME {
        return Err(RejectionKind::SpectatorLimitReached);
    }

    check_member_limit(state, pending_admissions)?;

    let member_id: MemberId = pending_admissions.get_next_member_id(state);
    pending_admissions.record_member(screen_name, MemberRoleKind::Spectator);

    Ok(JoinAdmission {
        member_id,
        member_events: vec![MemberEvent::Joined {
            member_id,
            screen_name: String::from(screen_name),
            role: MemberRoleKind::Spectator,
            loadout: None,
            team: None,
        }],
    })
}

pub fn admit_respawn(
    state: &GameState,
    pending_admissions: &mut PendingAdmissions,
    member_id: MemberId,
    loadout: Loadout,
    team_choice: TeamChoiceKind,
) -> Result<MemberEvent, RejectionKind> {
    let member: &Member = get_respawning_member(state, pending_admissions, member_id)?;

    check_spawn_slot(state, pending_admissions)?;

    if is_round_in_progress(state) {
        return Err(RejectionKind::RoundInProgress);
    }

    let team: Option<TeamKind> = resolve_team(state, pending_admissions, team_choice, member.team, Some(member_id))?;
    pending_admissions.record_spawn(member_id, team);

    Ok(MemberEvent::SpawnRequested {
        member_id,
        loadout,
        team,
    })
}

fn check_new_member(
    state: &GameState,
    pending_admissions: &PendingAdmissions,
    screen_name: &str,
) -> Result<(), RejectionKind> {
    if pending_admissions.is_screen_name_taken(state, screen_name) {
        return Err(RejectionKind::ScreenNameTaken);
    }

    check_member_limit(state, pending_admissions)
}

fn check_member_limit(state: &GameState, pending_admissions: &PendingAdmissions) -> Result<(), RejectionKind> {
    let member_limit: u32 = protocol_limits::get_member_limit(state.settings.player_cap);

    if pending_admissions.get_member_count(state) >= member_limit {
        return Err(RejectionKind::GameFull);
    }

    Ok(())
}

fn check_spawn_slot(state: &GameState, pending_admissions: &PendingAdmissions) -> Result<(), RejectionKind> {
    if pending_admissions.get_spawning_count(state) >= u32::from(state.settings.player_cap) {
        return Err(RejectionKind::GameFull);
    }

    Ok(())
}

/// A member whose join is still pending has not died either.
fn get_respawning_member<'state>(
    state: &'state GameState,
    pending_admissions: &PendingAdmissions,
    member_id: MemberId,
) -> Result<&'state Member, RejectionKind> {
    let Some(member): Option<&Member> = state.members.get(&member_id) else {
        let is_pending_join: bool = member_id >= state.next_member_id;

        return match is_pending_join {
            true => Err(RejectionKind::NotDead),
            false => Err(RejectionKind::NotInGame),
        };
    };

    if member.organism.is_some() || pending_admissions.is_spawn_pending(member_id) {
        return Err(RejectionKind::NotDead);
    }

    Ok(member)
}

fn is_round_in_progress(state: &GameState) -> bool {
    state.round.is_some_and(|round| matches!(round.phase, RoundPhase::Playing | RoundPhase::PostRound))
}

/// `None` outside skirmish; `Auto` takes the first smallest team in team order.
fn resolve_team(
    state: &GameState,
    pending_admissions: &PendingAdmissions,
    team_choice: TeamChoiceKind,
    current_team: Option<TeamKind>,
    excluded_member_id: Option<MemberId>,
) -> Result<Option<TeamKind>, RejectionKind> {
    let Some(team_count): Option<u8> = state.settings.team_count else {
        return Ok(None);
    };

    let team_sizes: TeamSizes = pending_admissions.get_team_sizes(state, team_count, excluded_member_id);
    let smallest_team: Option<TeamKind> = team_sizes.get_smallest_teams().first().copied();

    let TeamChoiceKind::Team(requested_team) = team_choice else {
        return Ok(smallest_team);
    };

    let team_choice_check: Result<(), TeamChoiceRejectionKind> =
        team_assignment::check_team_choice(&team_sizes, current_team, requested_team);

    match team_choice_check {
        Ok(()) => Ok(Some(requested_team)),
        Err(TeamChoiceRejectionKind::TeamUnbalanced { smaller_team }) => Err(RejectionKind::TeamUnbalanced {
            requested: requested_team,
            smaller: smaller_team,
        }),
        Err(TeamChoiceRejectionKind::TeamNotInGame) => Err(RejectionKind::TeamUnbalanced {
            requested: requested_team,
            smaller: smallest_team.unwrap_or(requested_team),
        }),
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib game::membership`

Expected: `test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 47 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/game/mod.rs server/src/game/membership.rs
git commit -m "join spectate and respawn admission"
```

### Task 16: Bundle builder

**Files:**
- Modify: `shared/src/ability/ability_model.rs`
- Modify: `shared/src/protocol/message_in.rs`
- Create: `server/src/game/bundle_builder.rs`
- Modify: `server/src/game/mod.rs`
- Test: inline `mod tests` in `shared/src/ability/ability_model.rs` and `server/src/game/bundle_builder.rs`

Admitted member events in arrival order and the latest input per member become the bundle for the next tick (protocol.md, input bundle; server.md, input validation). Presses received during the tick are OR-ed, the aim comes from the input which pressed a shot slot, and a member appears in `player_inputs` only when its rounded cursor moved or it pressed. Inputs from members without an organism, not after the last accepted `client_tick`, or sampled before the member's spawn are dropped; a `client_tick` ahead of the server is reported for the game task to close the connection. The shot-slot test which `TryFrom<PlayerInputSerialIn>` already makes moves first into `AbilityPressSet::has_shot_slot_press`, so the conversion and the builder share one definition.

- [ ] **Step 1: Write the failing test of the shot-slot predicate**

In `shared/src/ability/ability_model.rs`, append inside `mod tests`, directly above its closing brace:

```rust
    #[test]
    fn has_shot_slot_press_holds_for_a_first_or_second_press() {
        assert!(AbilityPressSet::FIRST.has_shot_slot_press());
        assert!(AbilityPressSet::SECOND.with(AbilityPressSet::THIRD).has_shot_slot_press());
        assert!(!AbilityPressSet::THIRD.with(AbilityPressSet::FOURTH).has_shot_slot_press());
        assert!(!AbilityPressSet::NONE.has_shot_slot_press());
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p shared --lib ability::ability_model`

Expected: compile failure, first line ``error[E0599]: no method named `has_shot_slot_press` found for struct `AbilityPressSet` in the current scope``.

- [ ] **Step 3: Implement the predicate and use it in the input conversion**

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

    /// The first and second slots carry the shot abilities.
    pub fn has_shot_slot_press(self) -> bool {
        self.contains(AbilityPressSet::FIRST) || self.contains(AbilityPressSet::SECOND)
    }
```

In `shared/src/protocol/message_in.rs`, replace:

```rust
        let has_shot_slot_press: bool =
            ability_presses.contains(AbilityPressSet::FIRST) || ability_presses.contains(AbilityPressSet::SECOND);

        if player_input_serial_in.aim.is_some() && !has_shot_slot_press {
```

with:

```rust
        if player_input_serial_in.aim.is_some() && !ability_presses.has_shot_slot_press() {
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p shared --lib -- ability::ability_model protocol::message_in`

Expected: `test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 303 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add shared/src/ability/ability_model.rs shared/src/protocol/message_in.rs
git commit -m "shot slot press predicate"
```

- [ ] **Step 6: Write the failing test of the bundle builder**

Create `server/src/game/bundle_builder.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::GameModeKind;
    use shared::member::{Member, MemberRoleKind, Score};

    const MEMBER_ID: MemberId = MemberId(0);
    const SPAWN_CURSOR: WorldPoint = WorldPoint { x: 100, y: 200 };

    fn create_state_with_organism(tick: Tick) -> GameState {
        let mut state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        state.members.insert(
            MEMBER_ID,
            Member {
                member_id: MEMBER_ID,
                screen_name: String::from("Blob"),
                role: MemberRoleKind::Participant,
                loadout: Some(test_fixture::create_loadout()),
                team: None,
                score: Score::zero(),
                organism: Some(Organism::new(SPAWN_CURSOR)),
            },
        );
        state.next_member_id = MemberId(1);
        state.tick = tick;

        state
    }

    fn create_builder_with_spawn(spawn_tick: Tick) -> BundleBuilder {
        let mut bundle_builder: BundleBuilder = BundleBuilder::new();
        bundle_builder.record_spawn(MEMBER_ID, SPAWN_CURSOR, spawn_tick);

        bundle_builder
    }

    fn create_input(client_tick: u32, cursor: WorldPoint, ability_presses: AbilityPressSet) -> PlayerInput {
        PlayerInput {
            client_tick: Tick(client_tick),
            cursor: cursor.to_subpixel_point(),
            ability_presses,
            aim: None,
        }
    }

    #[test]
    fn build_puts_the_queued_member_events_in_the_next_bundle_in_order() {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let mut bundle_builder: BundleBuilder = BundleBuilder::new();
        bundle_builder.queue_member_event(MemberEvent::Left { member_id: MemberId(4) });
        bundle_builder.queue_member_event(MemberEvent::Left { member_id: MemberId(2) });

        let built_bundle: BuiltBundle = bundle_builder.build(&state);

        assert_eq!(
            built_bundle.bundle,
            InputBundle {
                tick: Tick(1),
                member_events: vec![
                    MemberEvent::Left { member_id: MemberId(4) },
                    MemberEvent::Left { member_id: MemberId(2) },
                ],
                player_inputs: Vec::new(),
            },
        );
        assert_eq!(bundle_builder.build(&state).bundle.member_events, Vec::new());
    }

    #[test]
    fn build_applies_a_move_within_the_budget_without_a_correction() {
        let state: GameState = create_state_with_organism(Tick(5));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));
        let target: WorldPoint = WorldPoint { x: 103, y: 200 };

        bundle_builder.queue_input(&state, MEMBER_ID, create_input(5, target, AbilityPressSet::NONE));
        let built_bundle: BuiltBundle = bundle_builder.build(&state);

        assert_eq!(
            built_bundle.bundle.player_inputs,
            vec![PlayerTickInput {
                member_id: MEMBER_ID,
                cursor: target,
                ability_presses: AbilityPressSet::NONE,
                aim: None,
            }],
        );
        assert_eq!(built_bundle.cursor_corrections, Vec::new());
    }

    #[test]
    fn build_clamps_a_jump_and_reports_the_correction() {
        let state: GameState = create_state_with_organism(Tick(5));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));

        bundle_builder.queue_input(
            &state,
            MEMBER_ID,
            create_input(5, WorldPoint { x: 1100, y: 200 }, AbilityPressSet::NONE),
        );
        let built_bundle: BuiltBundle = bundle_builder.build(&state);

        assert_eq!(
            built_bundle.cursor_corrections,
            vec![CursorCorrection {
                member_id: MEMBER_ID,
                client_tick: Tick(5),
                cursor: SubpixelPoint {
                    x: 100 * 1024 + 3174,
                    y: 200 * 1024,
                },
            }],
        );
        assert_eq!(
            built_bundle.bundle.player_inputs[0].cursor,
            WorldPoint { x: 103, y: 200 }
        );
    }

    #[test]
    fn build_accumulates_budget_over_ticks_without_input() {
        let state: GameState = create_state_with_organism(Tick(5));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));
        bundle_builder.build(&state);
        bundle_builder.build(&state);

        bundle_builder.queue_input(
            &state,
            MEMBER_ID,
            create_input(5, WorldPoint { x: 109, y: 200 }, AbilityPressSet::NONE),
        );
        let built_bundle: BuiltBundle = bundle_builder.build(&state);

        assert_eq!(built_bundle.cursor_corrections, Vec::new());
        assert_eq!(
            built_bundle.bundle.player_inputs[0].cursor,
            WorldPoint { x: 109, y: 200 }
        );
    }

    #[test]
    fn build_leaves_out_a_member_which_neither_moved_nor_pressed() {
        let state: GameState = create_state_with_organism(Tick(5));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));

        bundle_builder.queue_input(&state, MEMBER_ID, create_input(5, SPAWN_CURSOR, AbilityPressSet::NONE));

        assert_eq!(bundle_builder.build(&state).bundle.player_inputs, Vec::new());
    }

    #[test]
    fn build_ors_the_presses_of_a_tick_and_keeps_the_aim_of_the_shot_press() {
        let state: GameState = create_state_with_organism(Tick(7));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));
        let aim: AimVector = AimVector { x: 12, y: -3 };
        let mut shot_input: PlayerInput = create_input(5, SPAWN_CURSOR, AbilityPressSet::FIRST);
        shot_input.aim = Some(aim);

        bundle_builder.queue_input(&state, MEMBER_ID, shot_input);
        bundle_builder.queue_input(
            &state,
            MEMBER_ID,
            create_input(6, SPAWN_CURSOR, AbilityPressSet::FOURTH),
        );
        let built_bundle: BuiltBundle = bundle_builder.build(&state);

        assert_eq!(
            built_bundle.bundle.player_inputs,
            vec![PlayerTickInput {
                member_id: MEMBER_ID,
                cursor: SPAWN_CURSOR,
                ability_presses: AbilityPressSet::FIRST.with(AbilityPressSet::FOURTH),
                aim: Some(aim),
            }],
        );
    }

    #[test]
    fn queue_input_rejects_a_client_tick_ahead_of_the_server() {
        let state: GameState = create_state_with_organism(Tick(5));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));

        assert_eq!(
            bundle_builder.queue_input(&state, MEMBER_ID, create_input(6, SPAWN_CURSOR, AbilityPressSet::NONE)),
            InputAcceptanceKind::AheadOfServer,
        );
    }

    #[test]
    fn queue_input_drops_a_client_tick_not_after_the_last_accepted_one() {
        let state: GameState = create_state_with_organism(Tick(9));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));

        assert_eq!(
            bundle_builder.queue_input(&state, MEMBER_ID, create_input(8, SPAWN_CURSOR, AbilityPressSet::NONE)),
            InputAcceptanceKind::Accepted,
        );
        assert_eq!(
            bundle_builder.queue_input(&state, MEMBER_ID, create_input(8, SPAWN_CURSOR, AbilityPressSet::NONE)),
            InputAcceptanceKind::Stale,
        );
        assert_eq!(
            bundle_builder.queue_input(&state, MEMBER_ID, create_input(7, SPAWN_CURSOR, AbilityPressSet::NONE)),
            InputAcceptanceKind::Stale,
        );
    }

    #[test]
    fn queue_input_ignores_an_input_sampled_before_the_spawn() {
        let state: GameState = create_state_with_organism(Tick(9));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(8));

        assert_eq!(
            bundle_builder.queue_input(&state, MEMBER_ID, create_input(7, SPAWN_CURSOR, AbilityPressSet::FIRST)),
            InputAcceptanceKind::BeforeSpawn,
        );
        assert_eq!(bundle_builder.build(&state).bundle.player_inputs, Vec::new());
    }

    #[test]
    fn queue_input_ignores_a_member_without_an_organism() {
        let mut state: GameState = create_state_with_organism(Tick(9));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(5));
        state.members.get_mut(&MEMBER_ID).unwrap().organism = None;

        assert_eq!(
            bundle_builder.queue_input(&state, MEMBER_ID, create_input(8, SPAWN_CURSOR, AbilityPressSet::FIRST)),
            InputAcceptanceKind::WithoutOrganism,
        );
        assert_eq!(
            bundle_builder.queue_input(
                &state,
                MemberId(7),
                create_input(8, SPAWN_CURSOR, AbilityPressSet::FIRST)
            ),
            InputAcceptanceKind::WithoutOrganism,
        );
    }

    #[test]
    fn record_spawn_resets_the_clamp_to_the_spawn_cursor_with_an_empty_budget() {
        let state: GameState = create_state_with_organism(Tick(5));
        let mut bundle_builder: BundleBuilder = create_builder_with_spawn(Tick(1));
        bundle_builder.build(&state);
        bundle_builder.build(&state);

        bundle_builder.record_spawn(MEMBER_ID, SPAWN_CURSOR, Tick(5));
        bundle_builder.queue_input(
            &state,
            MEMBER_ID,
            create_input(5, WorldPoint { x: 104, y: 200 }, AbilityPressSet::NONE),
        );
        let built_bundle: BuiltBundle = bundle_builder.build(&state);

        assert_eq!(
            built_bundle.bundle.player_inputs[0].cursor,
            WorldPoint { x: 103, y: 200 }
        );
        assert_eq!(built_bundle.cursor_corrections.len(), 1);
    }

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
}
```

Replace the whole contents of `server/src/game/mod.rs` with:

```rust
pub mod bundle_builder;
pub mod cursor_clamp;
pub mod game_handle;
pub mod game_registry;
pub mod game_runtime;
pub mod membership;
pub mod pending_admissions;
// Fixtures shared by the unit tests of several modules; absent from every non-test build.
#[cfg(test)]
pub mod test_fixture;

pub use bundle_builder::*;
pub use cursor_clamp::*;
pub use game_handle::*;
pub use game_registry::*;
pub use game_runtime::*;
pub use membership::*;
pub use pending_admissions::*;
```

- [ ] **Step 7: Run the test to verify it fails**

Run: `cargo test -p server --lib game::bundle_builder`

Expected: compile failure, first line ``error[E0425]: cannot find type `MemberId` in this scope``.

- [ ] **Step 8: Implement**

In `server/src/game/bundle_builder.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::collections::BTreeMap;

use shared::ability::{AbilityPressSet, AimVector};
use shared::game::{GameState, InputBundle, MemberEvent, PlayerInput, PlayerTickInput, Tick};
use shared::geometry;
use shared::geometry::{SubpixelPoint, WorldPoint};
use shared::member::MemberId;
use shared::organism::Organism;

use crate::game::{CursorClamp, CursorClampKind};

/// Turns the member events and inputs received between two ticks into the next bundle.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BundleBuilder {
    member_events: Vec<MemberEvent>,
    member_inputs: BTreeMap<MemberId, MemberInputState>,
    pending_inputs: BTreeMap<MemberId, PendingInput>,
}

impl BundleBuilder {
    pub fn new() -> BundleBuilder {
        BundleBuilder::default()
    }

    /// Applied in arrival order.
    pub fn queue_member_event(&mut self, member_event: MemberEvent) {
        self.member_events.push(member_event);
    }

    pub fn queue_input(&mut self, state: &GameState, member_id: MemberId, input: PlayerInput) -> InputAcceptanceKind {
        if input.client_tick > state.tick {
            return InputAcceptanceKind::AheadOfServer;
        }

        let has_organism: bool = state.members.get(&member_id).is_some_and(|member| member.organism.is_some());
        let member_input: Option<&mut MemberInputState> = self.member_inputs.get_mut(&member_id);
        let Some(member_input) = member_input.filter(|_| has_organism) else {
            return InputAcceptanceKind::WithoutOrganism;
        };

        if member_input.last_client_tick.is_some_and(|last_client_tick| input.client_tick <= last_client_tick) {
            return InputAcceptanceKind::Stale;
        }

        if input.client_tick < member_input.spawn_tick {
            return InputAcceptanceKind::BeforeSpawn;
        }

        member_input.last_client_tick = Some(input.client_tick);
        self.pending_inputs.entry(member_id).or_insert_with(PendingInput::new).merge(input);

        InputAcceptanceKind::Accepted
    }

    pub fn record_spawn(&mut self, member_id: MemberId, cursor: WorldPoint, spawn_tick: Tick) {
        let last_client_tick: Option<Tick> =
            self.member_inputs.get(&member_id).and_then(|member_input| member_input.last_client_tick);

        self.member_inputs.insert(
            member_id,
            MemberInputState {
                clamp: CursorClamp::new(cursor.to_subpixel_point()),
                spawn_tick,
                last_client_tick,
            },
        );
        self.pending_inputs.remove(&member_id);
    }

    pub fn forget_member(&mut self, member_id: MemberId) {
        self.member_inputs.remove(&member_id);
        self.pending_inputs.remove(&member_id);
    }

    /// The bundle for the tick after `state`; a member appears in it only when its cursor moved or it pressed.
    pub fn build(&mut self, state: &GameState) -> BuiltBundle {
        let mut pending_inputs: BTreeMap<MemberId, PendingInput> = std::mem::take(&mut self.pending_inputs);
        let mut player_inputs: Vec<PlayerTickInput> = Vec::new();
        let mut cursor_corrections: Vec<CursorCorrection> = Vec::new();

        for (member_id, member_input) in &mut self.member_inputs {
            let organism: Option<&Organism> = state.members.get(member_id).and_then(|member| member.organism.as_ref());
            let Some(organism) = organism else {
                continue;
            };

            member_input.clamp.add_tick_budget();

            let Some(pending_input) = pending_inputs.remove(member_id) else {
                continue;
            };

            let clamp_kind: CursorClampKind = member_input.clamp.apply(pending_input.target);
            let applied_cursor: SubpixelPoint = member_input.clamp.applied_cursor();

            if clamp_kind == CursorClampKind::Clamped {
                cursor_corrections.push(CursorCorrection {
                    member_id: *member_id,
                    client_tick: pending_input.client_tick,
                    cursor: applied_cursor,
                });
            }

            let cursor: WorldPoint = get_rounded_world_point(applied_cursor);
            let has_change: bool = cursor != organism.cursor || pending_input.ability_presses != AbilityPressSet::NONE;

            if has_change {
                player_inputs.push(PlayerTickInput {
                    member_id: *member_id,
                    cursor,
                    ability_presses: pending_input.ability_presses,
                    aim: pending_input.aim,
                });
            }
        }

        BuiltBundle {
            bundle: InputBundle {
                tick: state.tick.next(),
                member_events: std::mem::take(&mut self.member_events),
                player_inputs,
            },
            cursor_corrections,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputAcceptanceKind {
    Accepted,
    /// Sampled after a bundle the server has not built; a protocol violation.
    AheadOfServer,
    WithoutOrganism,
    /// Not after the member's last accepted input.
    Stale,
    /// Sampled before the client saw its spawn.
    BeforeSpawn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuiltBundle {
    pub bundle: InputBundle,
    pub cursor_corrections: Vec<CursorCorrection>,
}

/// Sent only to the clamped player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorCorrection {
    pub member_id: MemberId,
    pub client_tick: Tick,
    pub cursor: SubpixelPoint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MemberInputState {
    clamp: CursorClamp,
    spawn_tick: Tick,
    last_client_tick: Option<Tick>,
}

/// The inputs a member sent since the last bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PendingInput {
    client_tick: Tick,
    target: SubpixelPoint,
    ability_presses: AbilityPressSet,
    aim: Option<AimVector>,
}

impl PendingInput {
    fn new() -> PendingInput {
        PendingInput {
            client_tick: Tick(0),
            target: SubpixelPoint { x: 0, y: 0 },
            ability_presses: AbilityPressSet::NONE,
            aim: None,
        }
    }

    /// The latest cursor wins, presses accumulate, and the aim comes with the input which pressed a shot slot.
    fn merge(&mut self, input: PlayerInput) {
        self.client_tick = input.client_tick;
        self.target = input.cursor;

        if input.ability_presses.has_shot_slot_press() {
            self.aim = input.aim;
        }

        self.ability_presses = self.ability_presses.with(input.ability_presses);
    }
}

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

- [ ] **Step 9: Run the test to verify it passes**

Run: `cargo test -p server --lib game::bundle_builder`

Expected: `test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 63 filtered out`.

- [ ] **Step 10: Commit**

```sh
git status
git add server/src/game/mod.rs server/src/game/bundle_builder.rs
git commit -m "bundle builder"
```

### Task 17: Recipients and slow consumers

**Files:**
- Create: `server/src/game/recipients.rs`
- Modify: `server/src/game/mod.rs`
- Test: inline `mod tests` in `server/src/game/recipients.rs`

server.md (tasks and channels): membership frames go out with `try_send`. A full queue stops bundles to that member until it drains below half, when it gets a fresh snapshot; still full after `SLOW_CONSUMER_LIMIT = 10 s`, the member is told to close (`SlowConsumer`) and dropped. A closed queue drops the member. Requested snapshots wait for `SNAPSHOT_MINIMUM_INTERVAL_TICKS = 29` (protocol.md, checksums and resync).

- [ ] **Step 1: Write the failing test**

Create `server/src/game/recipients.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::socket::Membership;
    use crate::socket::connection_model;

    const MEMBER_ID: MemberId = MemberId(3);

    fn add_recipient(recipients: &mut Recipients, member_id: MemberId, snapshot_tick: Tick) -> Membership {
        let (endpoint, membership): (ConnectionEndpoint, Membership) =
            connection_model::create_membership(ConnectionId(u64::from(member_id.0)));
        recipients.add(member_id, endpoint, snapshot_tick);

        membership
    }

    fn fill_queue(recipients: &mut Recipients, now: Instant) {
        for _ in 0..=connection_model::MEMBERSHIP_CHANNEL_CAPACITY {
            recipients.broadcast(&Bytes::from_static(b"bundle"), now);
        }
    }

    fn drain(membership: &mut Membership) -> Vec<Bytes> {
        let mut frames: Vec<Bytes> = Vec::new();

        while let Ok(frame) = membership.outbound.try_recv() {
            frames.push(frame);
        }

        frames
    }

    #[test]
    fn broadcast_queues_each_frame_for_every_recipient() {
        let mut recipients: Recipients = Recipients::new();
        let mut first_membership: Membership = add_recipient(&mut recipients, MemberId(0), Tick(0));
        let mut second_membership: Membership = add_recipient(&mut recipients, MemberId(1), Tick(0));

        recipients.broadcast(&Bytes::from_static(b"bundle"), Instant::now());
        recipients.send_to_member(MemberId(1), Bytes::from_static(b"correction"), Instant::now());

        assert_eq!(drain(&mut first_membership), vec![Bytes::from_static(b"bundle")]);
        assert_eq!(
            drain(&mut second_membership),
            vec![Bytes::from_static(b"bundle"), Bytes::from_static(b"correction")],
        );
    }

    #[test]
    fn get_snapshot_member_ids_waits_for_the_minimum_interval_and_coalesces_requests() {
        let mut recipients: Recipients = Recipients::new();
        let _membership: Membership = add_recipient(&mut recipients, MEMBER_ID, Tick(10));

        recipients.request_snapshot(MEMBER_ID);
        recipients.request_snapshot(MEMBER_ID);

        assert_eq!(recipients.get_snapshot_member_ids(Tick(38)), Vec::new());
        assert_eq!(recipients.get_snapshot_member_ids(Tick(39)), vec![MEMBER_ID]);

        recipients.send_snapshot(MEMBER_ID, Bytes::from_static(b"snapshot"), Tick(39), Instant::now());

        assert_eq!(recipients.get_snapshot_member_ids(Tick(100)), Vec::new());
    }

    #[test]
    fn broadcast_stops_queueing_for_a_full_queue_until_it_drains_and_gets_a_snapshot() {
        let mut recipients: Recipients = Recipients::new();
        let mut membership: Membership = add_recipient(&mut recipients, MEMBER_ID, Tick(0));
        let now: Instant = Instant::now();
        fill_queue(&mut recipients, now);

        assert_eq!(recipients.get_snapshot_member_ids(Tick(1)), Vec::new());

        for _ in 0..connection_model::MEMBERSHIP_CHANNEL_CAPACITY / 2 + 1 {
            membership.outbound.try_recv().unwrap();
        }

        assert_eq!(recipients.get_snapshot_member_ids(Tick(1)), vec![MEMBER_ID]);

        drain(&mut membership);
        recipients.broadcast(&Bytes::from_static(b"skipped"), now);
        recipients.send_snapshot(MEMBER_ID, Bytes::from_static(b"snapshot"), Tick(1), now);
        recipients.broadcast(&Bytes::from_static(b"bundle"), now);

        assert_eq!(
            drain(&mut membership),
            vec![Bytes::from_static(b"snapshot"), Bytes::from_static(b"bundle")],
        );
        assert_eq!(recipients.remove_departed(now + SLOW_CONSUMER_LIMIT), Vec::new());
    }

    #[test]
    fn remove_departed_closes_a_slow_consumer_still_full_after_the_limit() {
        let mut recipients: Recipients = Recipients::new();
        let membership: Membership = add_recipient(&mut recipients, MEMBER_ID, Tick(0));
        let now: Instant = Instant::now();
        fill_queue(&mut recipients, now);

        assert_eq!(
            recipients.remove_departed(now + Duration::from_millis(9_999)),
            Vec::new()
        );
        assert_eq!(recipients.remove_departed(now + SLOW_CONSUMER_LIMIT), vec![MEMBER_ID]);
        assert!(!recipients.contains(MEMBER_ID));
        assert_eq!(
            *membership.control.borrow(),
            ConnectionControl::Close(CloseCauseKind::Reason(CloseReasonKind::SlowConsumer)),
        );
    }

    #[test]
    fn remove_departed_removes_a_recipient_whose_connection_is_gone() {
        let mut recipients: Recipients = Recipients::new();
        let membership: Membership = add_recipient(&mut recipients, MEMBER_ID, Tick(0));
        let _other_membership: Membership = add_recipient(&mut recipients, MemberId(4), Tick(0));

        drop(membership);

        assert_eq!(recipients.remove_departed(Instant::now()), vec![MEMBER_ID]);
        assert!(recipients.contains(MemberId(4)));
    }

    #[test]
    fn remove_connection_finds_the_member_of_a_connection() {
        let mut recipients: Recipients = Recipients::new();
        let _membership: Membership = add_recipient(&mut recipients, MEMBER_ID, Tick(0));

        assert_eq!(recipients.remove_connection(ConnectionId(9)), None);
        assert_eq!(recipients.remove_connection(ConnectionId(3)), Some(MEMBER_ID));
        assert!(recipients.is_empty());
    }
}
```

Replace the whole contents of `server/src/game/mod.rs` with:

```rust
pub mod bundle_builder;
pub mod cursor_clamp;
pub mod game_handle;
pub mod game_registry;
pub mod game_runtime;
pub mod membership;
pub mod pending_admissions;
pub mod recipients;
// Fixtures shared by the unit tests of several modules; absent from every non-test build.
#[cfg(test)]
pub mod test_fixture;

pub use bundle_builder::*;
pub use cursor_clamp::*;
pub use game_handle::*;
pub use game_registry::*;
pub use game_runtime::*;
pub use membership::*;
pub use pending_admissions::*;
pub use recipients::*;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib game::recipients`

Expected: compile failure, first line ``error[E0425]: cannot find type `MemberId` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/game/recipients.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::mpsc::error::TrySendError;

use shared::game::Tick;
use shared::member::MemberId;
use shared::protocol::CloseReasonKind;

use crate::game::ConnectionEndpoint;
use crate::socket::{CloseCauseKind, ConnectionControl, ConnectionId};

pub const SLOW_CONSUMER_LIMIT: Duration = Duration::from_secs(10);
pub const SNAPSHOT_MINIMUM_INTERVAL_TICKS: u32 = 29;

/// The member connections of a game, each fed through its own bounded membership queue.
#[derive(Default)]
pub struct Recipients {
    recipients: BTreeMap<MemberId, Recipient>,
}

impl Recipients {
    pub fn new() -> Recipients {
        Recipients::default()
    }

    pub fn is_empty(&self) -> bool {
        self.recipients.is_empty()
    }

    pub fn contains(&self, member_id: MemberId) -> bool {
        self.recipients.contains_key(&member_id)
    }

    /// `snapshot_tick` is the tick of the snapshot already queued for the member.
    pub fn add(&mut self, member_id: MemberId, endpoint: ConnectionEndpoint, snapshot_tick: Tick) {
        self.recipients.insert(
            member_id,
            Recipient {
                endpoint,
                delivery: DeliveryKind::Streaming,
                last_snapshot_tick: snapshot_tick,
                is_snapshot_requested: false,
            },
        );
    }

    pub fn remove(&mut self, member_id: MemberId) -> Option<ConnectionEndpoint> {
        self.recipients.remove(&member_id).map(|recipient| recipient.endpoint)
    }

    /// The connection closes with `cause`.
    pub fn close_member(&mut self, member_id: MemberId, cause: CloseCauseKind) -> Option<ConnectionEndpoint> {
        let endpoint: ConnectionEndpoint = self.remove(member_id)?;
        let _ = endpoint.control.send(ConnectionControl::Close(cause));

        Some(endpoint)
    }

    pub fn remove_connection(&mut self, connection_id: ConnectionId) -> Option<MemberId> {
        let member_id: MemberId = self
            .recipients
            .iter()
            .find(|(_, recipient)| recipient.endpoint.connection_id == connection_id)
            .map(|(member_id, _)| *member_id)?;
        self.recipients.remove(&member_id);

        Some(member_id)
    }

    /// Skips recipients awaiting a snapshot.
    pub fn send_to_member(&mut self, member_id: MemberId, frame: Bytes, now: Instant) {
        let Some(recipient) = self.recipients.get_mut(&member_id) else {
            return;
        };

        recipient.send(frame, now);
    }

    /// Skips recipients awaiting a snapshot.
    pub fn broadcast(&mut self, frame: &Bytes, now: Instant) {
        for recipient in self.recipients.values_mut() {
            recipient.send(frame.clone(), now);
        }
    }

    /// Repeats coalesce.
    pub fn request_snapshot(&mut self, member_id: MemberId) {
        let Some(recipient) = self.recipients.get_mut(&member_id) else {
            return;
        };

        recipient.is_snapshot_requested = true;
    }

    /// Requested snapshots whose minimum interval has passed, and slow consumers whose queue has drained below half.
    pub fn get_snapshot_member_ids(&self, tick: Tick) -> Vec<MemberId> {
        self.recipients
            .iter()
            .filter(|(_, recipient)| recipient.is_snapshot_due(tick))
            .map(|(member_id, _)| *member_id)
            .collect()
    }

    /// Bundles to the member resume after the snapshot.
    pub fn send_snapshot(&mut self, member_id: MemberId, snapshot_frame: Bytes, tick: Tick, now: Instant) {
        let Some(recipient) = self.recipients.get_mut(&member_id) else {
            return;
        };

        recipient.delivery = DeliveryKind::Streaming;
        recipient.is_snapshot_requested = false;
        recipient.last_snapshot_tick = tick;
        recipient.send(snapshot_frame, now);
    }

    /// Removes recipients whose connection is gone and slow consumers past the limit, which are told to close.
    pub fn remove_departed(&mut self, now: Instant) -> Vec<MemberId> {
        let departures: Vec<(MemberId, DepartureKind)> = self
            .recipients
            .iter()
            .filter_map(|(member_id, recipient)| recipient.get_departure(now).map(|departure| (*member_id, departure)))
            .collect();

        for (member_id, departure) in &departures {
            match departure {
                DepartureKind::ConnectionGone => self.remove(*member_id),
                DepartureKind::SlowConsumer => {
                    self.close_member(*member_id, CloseCauseKind::Reason(CloseReasonKind::SlowConsumer))
                }
            };
        }

        departures.into_iter().map(|(member_id, _)| member_id).collect()
    }
}

struct Recipient {
    endpoint: ConnectionEndpoint,
    delivery: DeliveryKind,
    last_snapshot_tick: Tick,
    is_snapshot_requested: bool,
}

impl Recipient {
    fn send(&mut self, frame: Bytes, now: Instant) {
        if self.delivery != DeliveryKind::Streaming {
            return;
        }

        let send_result: Result<(), TrySendError<Bytes>> = self.endpoint.outbound.try_send(frame);

        if let Err(TrySendError::Full(_)) = send_result {
            self.delivery = DeliveryKind::AwaitingDrain { full_since: now };
        }
    }

    fn is_snapshot_due(&self, tick: Tick) -> bool {
        match self.delivery {
            DeliveryKind::Streaming => {
                self.is_snapshot_requested
                    && tick.ticks_since(self.last_snapshot_tick) >= SNAPSHOT_MINIMUM_INTERVAL_TICKS
            }
            DeliveryKind::AwaitingDrain { .. } => {
                self.endpoint.outbound.capacity() > self.endpoint.outbound.max_capacity() / 2
            }
        }
    }

    fn get_departure(&self, now: Instant) -> Option<DepartureKind> {
        if self.endpoint.outbound.is_closed() {
            return Some(DepartureKind::ConnectionGone);
        }

        let DeliveryKind::AwaitingDrain { full_since } = self.delivery else {
            return None;
        };

        if now.saturating_duration_since(full_since) >= SLOW_CONSUMER_LIMIT {
            return Some(DepartureKind::SlowConsumer);
        }

        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeliveryKind {
    Streaming,
    /// The queue was full; nothing more is queued until a fresh snapshot.
    AwaitingDrain {
        full_since: Instant,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DepartureKind {
    ConnectionGone,
    SlowConsumer,
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib game::recipients`

Expected: `test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 75 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/game/mod.rs server/src/game/recipients.rs
git commit -m "recipients and slow consumers"
```

### Task 18: Replay capture

**Files:**
- Create: `server/src/game/replay_capture.rs`
- Modify: `server/src/game/mod.rs`
- Test: inline `mod tests` in `server/src/game/replay_capture.rs`

server.md (game task, replay capture): a writer task per game on `GAME_RUNTIME` appends the version prefix, the header record and one bundle record per tick to `<directory>/<game_id>.replay`, through phase 4's `replay::get_header_record_bytes` and `replay::get_bundle_record_bytes`, flushing after each. A full channel abandons the capture.

- [ ] **Step 1: Write the failing test**

Create `server/src/game/replay_capture.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::{GameModeKind, GameSettings, InputBundle, MemberEvent, Tick};
    use shared::member::MemberId;
    use shared::protocol::GameSettingsSerialOut;
    use shared::replay::{ReplayHeader, ReplayLog};

    fn create_bundle(tick: u32) -> InputBundle {
        InputBundle {
            tick: Tick(tick),
            member_events: vec![MemberEvent::Left {
                member_id: MemberId(tick),
            }],
            player_inputs: Vec::new(),
        }
    }

    #[test]
    fn start_writes_the_header_and_every_recorded_bundle() {
        let directory: PathBuf = test_fixture::create_temporary_directory("replay-capture");
        let settings: GameSettings = test_fixture::create_settings(GameModeKind::FreeForAll, 8);
        let header_serial_out: ReplayHeaderSerialOut = ReplayHeaderSerialOut {
            settings: GameSettingsSerialOut::from(&settings),
            seed: 77,
        };

        let (mut replay_capture, writer_handle): (ReplayCapture, JoinHandle<()>) =
            ReplayCapture::start(&directory.join("nested"), GameId(5), header_serial_out);
        replay_capture.record_bundle(InputBundleSerialOut::from(&create_bundle(1)));
        replay_capture.record_bundle(InputBundleSerialOut::from(&create_bundle(2)));
        drop(replay_capture);
        game_runtime::GAME_RUNTIME.block_on(writer_handle).unwrap();

        let replay_bytes: Vec<u8> = std::fs::read(directory.join("nested/5.replay")).unwrap();

        assert_eq!(
            replay::read_replay(&replay_bytes).unwrap(),
            ReplayLog {
                header: ReplayHeader { settings, seed: 77 },
                input_bundles: vec![create_bundle(1), create_bundle(2)],
            },
        );
    }
}
```

Replace the whole contents of `server/src/game/mod.rs` with:

```rust
pub mod bundle_builder;
pub mod cursor_clamp;
pub mod game_handle;
pub mod game_registry;
pub mod game_runtime;
pub mod membership;
pub mod pending_admissions;
pub mod recipients;
pub mod replay_capture;
// Fixtures shared by the unit tests of several modules; absent from every non-test build.
#[cfg(test)]
pub mod test_fixture;

pub use bundle_builder::*;
pub use cursor_clamp::*;
pub use game_handle::*;
pub use game_registry::*;
pub use game_runtime::*;
pub use membership::*;
pub use pending_admissions::*;
pub use recipients::*;
pub use replay_capture::*;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib game::replay_capture`

Expected: compile failure, first line ``error[E0425]: cannot find type `PathBuf` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/game/replay_capture.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::path::{Path, PathBuf};

use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::task::JoinHandle;

use shared::error::AppError;
use shared::game::GameId;
use shared::protocol::{InputBundleSerialOut, ReplayHeaderSerialOut};
use shared::replay;

use crate::game::game_runtime;

pub const REPLAY_CAPTURE_CHANNEL_CAPACITY: usize = 1024;

/// Appends a game's replay records to `<directory>/<game_id>.replay` from a writer task.
pub struct ReplayCapture {
    game_id: GameId,
    /// None once the capture is abandoned.
    records: Option<mpsc::Sender<ReplayRecord>>,
}

impl ReplayCapture {
    pub fn start(
        directory: &Path,
        game_id: GameId,
        header_serial_out: ReplayHeaderSerialOut,
    ) -> (ReplayCapture, JoinHandle<()>) {
        let (record_sender, record_receiver): (mpsc::Sender<ReplayRecord>, mpsc::Receiver<ReplayRecord>) =
            mpsc::channel(REPLAY_CAPTURE_CHANNEL_CAPACITY);
        let path: PathBuf = directory.join(format!("{}.replay", game_id.0));
        let mut replay_capture: ReplayCapture = ReplayCapture {
            game_id,
            records: Some(record_sender),
        };

        replay_capture.send(ReplayRecord::Header(header_serial_out));
        let writer_handle: JoinHandle<()> =
            game_runtime::GAME_RUNTIME.spawn(run_replay_writer(game_id, path, record_receiver));

        (replay_capture, writer_handle)
    }

    pub fn record_bundle(&mut self, bundle_serial_out: InputBundleSerialOut) {
        self.send(ReplayRecord::Bundle(bundle_serial_out));
    }

    /// A full or closed channel abandons the capture.
    fn send(&mut self, record: ReplayRecord) {
        let Some(records) = &self.records else {
            return;
        };

        let send_result: Result<(), TrySendError<ReplayRecord>> = records.try_send(record);

        if send_result.is_err() {
            log::warn!("replay capture abandoned; [game_id={}]", self.game_id.0);
            self.records = None;
        }
    }
}

enum ReplayRecord {
    Header(ReplayHeaderSerialOut),
    Bundle(InputBundleSerialOut),
}

async fn run_replay_writer(game_id: GameId, path: PathBuf, record_receiver: mpsc::Receiver<ReplayRecord>) {
    let write_result: Result<(), AppError> = write_replay_records(&path, record_receiver).await;

    if let Err(error) = write_result {
        log::error!("replay capture failed; [game_id={} error={error}]", game_id.0);
    }
}

async fn write_replay_records(path: &Path, mut record_receiver: mpsc::Receiver<ReplayRecord>) -> Result<(), AppError> {
    if let Some(directory) = path.parent() {
        tokio::fs::create_dir_all(directory).await?;
    }

    let mut file: File = File::create(path).await?;
    file.write_all(&replay::get_version_prefix()).await?;

    while let Some(record) = record_receiver.recv().await {
        let record_bytes: Vec<u8> = match &record {
            ReplayRecord::Header(header_serial_out) => replay::get_header_record_bytes(header_serial_out),
            ReplayRecord::Bundle(bundle_serial_out) => replay::get_bundle_record_bytes(bundle_serial_out),
        };

        file.write_all(&record_bytes).await?;
        // The file is a valid replay prefix at every moment.
        file.flush().await?;
    }

    Ok(())
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib game::replay_capture`

Expected: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 81 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/game/mod.rs server/src/game/replay_capture.rs
git commit -m "replay capture"
```

### Task 19: Game task

**Files:**
- Create: `server/src/game/game_task.rs`
- Modify: `server/src/game/mod.rs`
- Test: inline `mod tests` in `server/src/game/game_task.rs`

The tick loop of server.md (game task). Between ticks, an admitted join or spectate queues `JoinAccepted` and the snapshot of the last completed tick on the joiner's membership queue, then its events for the next bundle. Per tick: build the bundle and clear the pending admissions, `step`, fan out the bundle (and record it for replay), cursor corrections, the checksum every 14 ticks and due snapshots, remove departed recipients, react to `OrganismSpawned`, publish the summary, track emptiness. `GameTask` is synchronous, so the tests drive it without a runtime; `run_game_task` adds the interval with `MissedTickBehavior::Burst`, the lag reset after 43 late ticks and the drain of up to 4096 queued commands per tick.

- [ ] **Step 1: Write the failing test**

Create `server/src/game/game_task.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use crate::socket::{ConnectionControl, ConnectionId, Membership};
    use shared::ability::AbilityPressSet;
    use shared::game::{GameModeKind, PlayerInput};
    use shared::geometry::SubpixelPoint;
    use shared::member::{Joiner, MemberRoleKind};
    use shared::protocol;
    use shared::protocol::{GameSummarySerialOut, MemberEventSerialOut};

    const GAME_ID: GameId = GameId(7);
    const GRACE_PERIOD: Duration = Duration::from_millis(200);

    struct TestGame {
        game_task: GameTask,
        summary: watch::Receiver<GameSummary>,
    }

    fn create_test_game(player_cap: u8) -> TestGame {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, player_cap);
        let (summary_sender, summary_receiver): (watch::Sender<GameSummary>, watch::Receiver<GameSummary>) =
            watch::channel(GameSummary::from_game_state(GAME_ID, &state, false));

        TestGame {
            game_task: GameTask::new(GAME_ID, state, false, summary_sender, None, GRACE_PERIOD),
            summary: summary_receiver,
        }
    }

    fn join(
        game_task: &mut GameTask,
        connection_id: u64,
        screen_name: &str,
    ) -> (Result<MemberId, RejectionKind>, Membership) {
        let (endpoint, membership): (ConnectionEndpoint, Membership) =
            connection_model::create_membership(ConnectionId(connection_id));
        let (reply_sender, mut reply_receiver): (
            oneshot::Sender<Result<MemberId, RejectionKind>>,
            oneshot::Receiver<Result<MemberId, RejectionKind>>,
        ) = oneshot::channel();
        let joiner: Joiner = test_fixture::create_joiner(screen_name, TeamChoiceKind::Auto);

        game_task.handle_request(
            GameRequest::Join {
                endpoint,
                joiner,
                reply: reply_sender,
            },
            Instant::now(),
        );

        (reply_receiver.try_recv().unwrap(), membership)
    }

    fn receive_messages(membership: &mut Membership) -> Vec<MessageSerialOut> {
        let mut message_serial_outs: Vec<MessageSerialOut> = Vec::new();

        while let Ok(frame) = membership.outbound.try_recv() {
            message_serial_outs.push(protocol::decode_message_out(&frame).unwrap());
        }

        message_serial_outs
    }

    fn get_bundle_member_events(message_serial_outs: &[MessageSerialOut]) -> Vec<MemberEventSerialOut> {
        message_serial_outs
            .iter()
            .filter_map(|message_serial_out| match message_serial_out {
                MessageSerialOut::InputBundle(bundle_serial_out) => Some(bundle_serial_out.member_events.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    fn run_ticks(game_task: &mut GameTask, tick_count: u32) {
        for _ in 0..tick_count {
            assert_eq!(game_task.run_tick(Instant::now()), GameFlow::Continue);
        }
    }

    #[test]
    fn handle_request_join_queues_join_accepted_then_the_snapshot_before_the_joined_bundle() {
        let mut test_game: TestGame = create_test_game(8);

        let (reply, mut membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Blob");
        test_game.game_task.run_tick(Instant::now());
        let message_serial_outs: Vec<MessageSerialOut> = receive_messages(&mut membership);

        assert_eq!(reply, Ok(MemberId(0)));
        assert_eq!(
            message_serial_outs[0],
            MessageSerialOut::JoinAccepted {
                game_id: GAME_ID.0,
                member_id: 0,
            },
        );
        assert!(matches!(
            &message_serial_outs[1],
            MessageSerialOut::Snapshot(GameSnapshotSerialOut { game_id: 7, state }) if state.tick == 0 && state.members.is_empty()
        ));
        assert!(matches!(
            &message_serial_outs[2],
            MessageSerialOut::InputBundle(bundle_serial_out) if bundle_serial_out.tick == 1
        ));
        assert!(matches!(
            get_bundle_member_events(&message_serial_outs)[..],
            [
                MemberEventSerialOut::Joined { member_id: 0, .. },
                MemberEventSerialOut::SpawnRequested { member_id: 0, .. }
            ]
        ));
        assert_eq!(test_game.game_task.state.alive_organism_count(), 1);
    }

    #[test]
    fn handle_request_join_snapshots_the_state_of_the_last_completed_tick() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, _first_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "First");
        run_ticks(&mut test_game.game_task, 3);

        let (reply, mut membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 2, "Second");
        let message_serial_outs: Vec<MessageSerialOut> = receive_messages(&mut membership);

        assert_eq!(reply, Ok(MemberId(1)));
        assert!(matches!(
            &message_serial_outs[1],
            MessageSerialOut::Snapshot(GameSnapshotSerialOut { state, .. }) if state.tick == 3 && state.members.len() == 1
        ));
    }

    #[test]
    fn handle_request_join_answers_a_rejection_without_queueing_frames() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, _first_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Blob");

        let (reply, mut membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 2, "Blob");

        assert_eq!(reply, Err(RejectionKind::ScreenNameTaken));
        assert_eq!(receive_messages(&mut membership), Vec::new());
    }

    #[test]
    fn run_tick_sends_the_state_checksum_on_its_interval() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, mut membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Blob");

        run_ticks(&mut test_game.game_task, CHECKSUM_INTERVAL_TICKS);
        let checksum_messages: Vec<MessageSerialOut> = receive_messages(&mut membership)
            .into_iter()
            .filter(|message_serial_out| matches!(message_serial_out, MessageSerialOut::StateChecksum { .. }))
            .collect();

        assert_eq!(
            checksum_messages,
            vec![MessageSerialOut::StateChecksum {
                tick: CHECKSUM_INTERVAL_TICKS,
                checksum: state_checksum::get_state_checksum(&test_game.game_task.state),
            }],
        );
    }

    #[test]
    fn handle_request_leave_queues_left_and_stops_the_frames() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, mut leaving_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Leaver");
        let (_, mut staying_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 2, "Stayer");
        test_game.game_task.run_tick(Instant::now());
        receive_messages(&mut leaving_membership);
        receive_messages(&mut staying_membership);

        let (reply_sender, mut reply_receiver): (oneshot::Sender<()>, oneshot::Receiver<()>) = oneshot::channel();
        test_game.game_task.handle_request(
            GameRequest::Leave {
                member_id: MemberId(0),
                reply: reply_sender,
            },
            Instant::now(),
        );
        test_game.game_task.run_tick(Instant::now());

        assert_eq!(reply_receiver.try_recv(), Ok(()));
        assert_eq!(receive_messages(&mut leaving_membership), Vec::new());
        assert_eq!(
            get_bundle_member_events(&receive_messages(&mut staying_membership)),
            vec![MemberEventSerialOut::Left { member_id: 0 }],
        );
    }

    #[test]
    fn handle_request_connection_closed_queues_left_for_its_member() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, _leaving_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Leaver");
        let (_, mut staying_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 2, "Stayer");
        test_game.game_task.run_tick(Instant::now());
        receive_messages(&mut staying_membership);

        test_game.game_task.handle_request(
            GameRequest::ConnectionClosed {
                connection_id: ConnectionId(1),
            },
            Instant::now(),
        );
        test_game.game_task.run_tick(Instant::now());

        assert_eq!(
            get_bundle_member_events(&receive_messages(&mut staying_membership)),
            vec![MemberEventSerialOut::Left { member_id: 0 }],
        );
    }

    #[test]
    fn run_tick_queues_left_for_a_member_whose_connection_is_gone() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, leaving_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Leaver");
        let (_, mut staying_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 2, "Stayer");
        test_game.game_task.run_tick(Instant::now());
        receive_messages(&mut staying_membership);

        drop(leaving_membership);
        run_ticks(&mut test_game.game_task, 2);

        assert_eq!(
            get_bundle_member_events(&receive_messages(&mut staying_membership)),
            vec![MemberEventSerialOut::Left { member_id: 0 }],
        );
        assert_eq!(test_game.game_task.state.members.len(), 1);
    }

    #[test]
    fn handle_input_closes_a_connection_whose_client_tick_is_ahead_of_the_server() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, membership): (Result<MemberId, RejectionKind>, Membership) = join(&mut test_game.game_task, 1, "Blob");
        test_game.game_task.run_tick(Instant::now());

        test_game.game_task.handle_input(GameInput {
            member_id: MemberId(0),
            input: PlayerInput {
                client_tick: Tick(2),
                cursor: SubpixelPoint { x: 0, y: 0 },
                ability_presses: AbilityPressSet::NONE,
                aim: None,
            },
        });
        test_game.game_task.run_tick(Instant::now());

        assert_eq!(
            *membership.control.borrow(),
            ConnectionControl::Close(CloseCauseKind::ProtocolViolation(ProtocolViolationKind::Validation)),
        );
        assert!(test_game.game_task.state.members.is_empty());
    }

    #[test]
    fn run_tick_sends_a_cursor_correction_only_to_the_clamped_player() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, mut clamped_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Fast");
        let (_, mut other_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 2, "Other");
        test_game.game_task.run_tick(Instant::now());
        receive_messages(&mut clamped_membership);
        receive_messages(&mut other_membership);

        test_game.game_task.handle_input(GameInput {
            member_id: MemberId(0),
            input: PlayerInput {
                client_tick: Tick(1),
                cursor: SubpixelPoint { x: 0, y: 0 },
                ability_presses: AbilityPressSet::NONE,
                aim: None,
            },
        });
        test_game.game_task.run_tick(Instant::now());

        assert!(
            receive_messages(&mut clamped_membership).iter().any(|message_serial_out| matches!(
                message_serial_out,
                MessageSerialOut::CursorCorrected { client_tick: 1, .. }
            ))
        );
        assert!(
            !receive_messages(&mut other_membership)
                .iter()
                .any(|message_serial_out| matches!(message_serial_out, MessageSerialOut::CursorCorrected { .. }))
        );
    }

    #[test]
    fn handle_request_request_snapshot_sends_one_snapshot_after_the_minimum_interval() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, mut membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Blob");
        receive_messages(&mut membership);

        for _ in 0..2 {
            test_game
                .game_task
                .handle_request(GameRequest::RequestSnapshot { member_id: MemberId(0) }, Instant::now());
        }

        run_ticks(&mut test_game.game_task, 40);
        let snapshot_ticks: Vec<u32> = receive_messages(&mut membership)
            .iter()
            .filter_map(|message_serial_out| match message_serial_out {
                MessageSerialOut::Snapshot(snapshot_serial_out) => Some(snapshot_serial_out.state.tick),
                _ => None,
            })
            .collect();

        assert_eq!(snapshot_ticks, vec![29]);
    }

    #[test]
    fn handle_request_respawn_rejects_a_member_without_a_connection() {
        let mut test_game: TestGame = create_test_game(8);
        let (reply_sender, mut reply_receiver): (
            oneshot::Sender<Result<(), RejectionKind>>,
            oneshot::Receiver<Result<(), RejectionKind>>,
        ) = oneshot::channel();

        test_game.game_task.handle_request(
            GameRequest::Respawn {
                member_id: MemberId(0),
                loadout: test_fixture::create_loadout(),
                team: TeamChoiceKind::Auto,
                reply: reply_sender,
            },
            Instant::now(),
        );

        assert_eq!(reply_receiver.try_recv(), Ok(Err(RejectionKind::NotInGame)));
    }

    #[test]
    fn handle_request_spectate_adds_a_spectator() {
        let mut test_game: TestGame = create_test_game(8);
        let (endpoint, _membership): (ConnectionEndpoint, Membership) =
            connection_model::create_membership(ConnectionId(1));
        let (reply_sender, mut reply_receiver): (
            oneshot::Sender<Result<MemberId, RejectionKind>>,
            oneshot::Receiver<Result<MemberId, RejectionKind>>,
        ) = oneshot::channel();

        test_game.game_task.handle_request(
            GameRequest::Spectate {
                endpoint,
                screen_name: String::from("Watcher"),
                reply: reply_sender,
            },
            Instant::now(),
        );
        test_game.game_task.run_tick(Instant::now());

        assert_eq!(reply_receiver.try_recv(), Ok(Ok(MemberId(0))));
        assert_eq!(
            test_game.game_task.state.members[&MemberId(0)].role,
            MemberRoleKind::Spectator
        );
    }

    #[test]
    fn run_tick_updates_the_summary_when_it_changes() {
        let mut test_game: TestGame = create_test_game(8);
        let (_, _first_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Blob");
        test_game.summary.mark_unchanged();

        test_game.game_task.run_tick(Instant::now());

        assert!(test_game.summary.has_changed().unwrap());
        assert_eq!(
            GameSummarySerialOut::from(&*test_game.summary.borrow_and_update()).player_count,
            1,
        );

        test_game.game_task.run_tick(Instant::now());

        assert!(!test_game.summary.has_changed().unwrap());
    }

    #[test]
    fn get_flow_ends_an_empty_game_after_the_grace_period_and_a_join_cancels_it() {
        let mut test_game: TestGame = create_test_game(8);
        let start: Instant = Instant::now();
        test_game.game_task.run_tick(start);

        assert_eq!(
            test_game.game_task.get_flow(start + GRACE_PERIOD - Duration::from_millis(1)),
            GameFlow::Continue
        );
        assert_eq!(test_game.game_task.get_flow(start + GRACE_PERIOD), GameFlow::End);

        let (_, _first_membership): (Result<MemberId, RejectionKind>, Membership) =
            join(&mut test_game.game_task, 1, "Blob");

        assert_eq!(
            test_game.game_task.get_flow(start + GRACE_PERIOD * 2),
            GameFlow::Continue
        );
    }

    #[test]
    fn answer_closed_game_answers_game_not_found() {
        let (endpoint, _membership): (ConnectionEndpoint, Membership) =
            connection_model::create_membership(ConnectionId(1));
        let (reply_sender, mut reply_receiver): (
            oneshot::Sender<Result<MemberId, RejectionKind>>,
            oneshot::Receiver<Result<MemberId, RejectionKind>>,
        ) = oneshot::channel();

        answer_closed_game(GameRequest::Join {
            endpoint,
            joiner: test_fixture::create_joiner("Late", TeamChoiceKind::Auto),
            reply: reply_sender,
        });

        assert_eq!(reply_receiver.try_recv(), Ok(Err(RejectionKind::GameNotFound)));
    }

    #[test]
    fn observe_resets_after_more_than_the_threshold_of_consecutive_late_ticks() {
        let mut tick_lag: TickLag = TickLag::default();
        let tick_period: Duration = Duration::from_millis(70);
        let due_at: Instant = Instant::now();
        let late_fired_at: Instant = due_at + tick_period;

        for _ in 0..TICK_LAG_RESET_THRESHOLD {
            assert_eq!(tick_lag.observe(due_at, late_fired_at, tick_period), TickLagKind::Late);
        }

        assert_eq!(
            tick_lag.observe(due_at, late_fired_at, tick_period),
            TickLagKind::ResetDue
        );
        assert_eq!(tick_lag.observe(due_at, late_fired_at, tick_period), TickLagKind::Late);
        assert_eq!(
            tick_lag.observe(due_at, due_at + Duration::from_millis(69), tick_period),
            TickLagKind::OnTime
        );
    }
}
```

Replace the whole contents of `server/src/game/mod.rs` with:

```rust
pub mod bundle_builder;
pub mod cursor_clamp;
pub mod game_handle;
pub mod game_registry;
pub mod game_runtime;
pub mod game_task;
pub mod membership;
pub mod pending_admissions;
pub mod recipients;
pub mod replay_capture;
// Fixtures shared by the unit tests of several modules; absent from every non-test build.
#[cfg(test)]
pub mod test_fixture;

pub use bundle_builder::*;
pub use cursor_clamp::*;
pub use game_handle::*;
pub use game_registry::*;
pub use game_runtime::*;
pub use game_task::*;
pub use membership::*;
pub use pending_admissions::*;
pub use recipients::*;
pub use replay_capture::*;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib game::game_task`

Expected: compile failure, first line ``error[E0425]: cannot find type `GameId` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/game/game_task.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{Interval, MissedTickBehavior};

use shared::ability::Loadout;
use shared::game;
use shared::game::{GameId, GameState, GameSummary, MemberEvent, SimulationEvent, StepError, Tick};
use shared::member::{MemberId, TeamChoiceKind};
use shared::protocol::state_checksum;
use shared::protocol::{
    GameSnapshotSerialOut, GameStateSerialOut, InputBundleSerialOut, MessageSerialOut, RejectionKind,
    SubpixelPointSerial,
};

use crate::game::membership;
use crate::game::{
    BuiltBundle, BundleBuilder, ConnectionEndpoint, CursorCorrection, GameInput, GameRequest, InputAcceptanceKind,
    JoinAdmission, PendingAdmissions, Recipients, ReplayCapture,
};
use crate::socket::connection_model;
use crate::socket::{CloseCauseKind, ProtocolViolationKind};

pub const CHECKSUM_INTERVAL_TICKS: u32 = 14;
/// About 3 s of ticks.
pub const TICK_LAG_RESET_THRESHOLD: u32 = 43;
pub const COMMAND_DRAIN_LIMIT_PER_TICK: usize = 4096;

/// Owns one game's state and members; never awaits a send.
pub struct GameTask {
    game_id: GameId,
    state: GameState,
    is_secured: bool,
    summary: watch::Sender<GameSummary>,
    recipients: Recipients,
    pending_admissions: PendingAdmissions,
    bundle_builder: BundleBuilder,
    replay_capture: Option<ReplayCapture>,
    empty_game_grace_period: Duration,
    empty_since: Option<Instant>,
    stale_input_count: u64,
}

impl GameTask {
    pub fn new(
        game_id: GameId,
        state: GameState,
        is_secured: bool,
        summary: watch::Sender<GameSummary>,
        replay_capture: Option<ReplayCapture>,
        empty_game_grace_period: Duration,
    ) -> GameTask {
        GameTask {
            game_id,
            state,
            is_secured,
            summary,
            recipients: Recipients::new(),
            pending_admissions: PendingAdmissions::new(),
            bundle_builder: BundleBuilder::new(),
            replay_capture,
            empty_game_grace_period,
            empty_since: None,
            stale_input_count: 0,
        }
    }

    pub fn handle_request(&mut self, request: GameRequest, now: Instant) {
        match request {
            GameRequest::Join {
                endpoint,
                joiner,
                reply,
            } => {
                let admission: Result<JoinAdmission, RejectionKind> =
                    membership::admit_join(&self.state, &mut self.pending_admissions, &joiner);
                self.admit_member(admission, endpoint, reply, now);
            }
            GameRequest::Spectate {
                endpoint,
                screen_name,
                reply,
            } => {
                let admission: Result<JoinAdmission, RejectionKind> =
                    membership::admit_spectate(&self.state, &mut self.pending_admissions, &screen_name);
                self.admit_member(admission, endpoint, reply, now);
            }
            GameRequest::Respawn {
                member_id,
                loadout,
                team,
                reply,
            } => {
                let respawn: Result<(), RejectionKind> = self.respawn_member(member_id, loadout, team);
                let _ = reply.send(respawn);
            }
            GameRequest::UpdateAppearance { member_id, appearance } => {
                if self.recipients.contains(member_id) {
                    self.bundle_builder.queue_member_event(MemberEvent::AppearanceChanged { member_id, appearance });
                }
            }
            GameRequest::Leave { member_id, reply } => {
                self.remove_member(member_id);
                let _ = reply.send(());
            }
            GameRequest::ConnectionClosed { connection_id } => {
                let member_id: Option<MemberId> = self.recipients.remove_connection(connection_id);

                if let Some(member_id) = member_id {
                    self.queue_departure(member_id);
                }
            }
            GameRequest::RequestSnapshot { member_id } => {
                log::info!(
                    "resync requested; [game_id={} member_id={} tick={}]",
                    self.game_id.0,
                    member_id.0,
                    self.state.tick.0,
                );
                self.recipients.request_snapshot(member_id);
            }
        }
    }

    pub fn handle_input(&mut self, game_input: GameInput) {
        if !self.recipients.contains(game_input.member_id) {
            return;
        }

        let acceptance: InputAcceptanceKind =
            self.bundle_builder.queue_input(&self.state, game_input.member_id, game_input.input);

        match acceptance {
            InputAcceptanceKind::AheadOfServer => {
                let cause: CloseCauseKind = CloseCauseKind::ProtocolViolation(ProtocolViolationKind::Validation);
                self.recipients.close_member(game_input.member_id, cause);
                self.queue_departure(game_input.member_id);
            }
            InputAcceptanceKind::Stale => self.stale_input_count += 1,
            InputAcceptanceKind::Accepted | InputAcceptanceKind::WithoutOrganism | InputAcceptanceKind::BeforeSpawn => {
            }
        }
    }

    pub fn run_tick(&mut self, now: Instant) -> GameFlow {
        let built_bundle: BuiltBundle = self.bundle_builder.build(&self.state);
        self.pending_admissions.clear();

        let step_result: Result<Vec<SimulationEvent>, StepError> = game::step(&mut self.state, &built_bundle.bundle);
        let simulation_events: Vec<SimulationEvent> = match step_result {
            Ok(simulation_events) => simulation_events,
            Err(error) => {
                log::error!("game step failed; [game_id={} error={error:?}]", self.game_id.0);

                return GameFlow::End;
            }
        };

        self.send_tick_frames(built_bundle, now);

        let departed_member_ids: Vec<MemberId> = self.recipients.remove_departed(now);

        for member_id in departed_member_ids {
            self.queue_departure(member_id);
        }

        self.handle_simulation_events(&simulation_events);
        self.update_summary();
        self.update_empty_since(now);

        GameFlow::Continue
    }

    /// An empty game ends after its grace period.
    pub fn get_flow(&self, now: Instant) -> GameFlow {
        let is_expired: bool = self
            .empty_since
            .is_some_and(|empty_since| now.saturating_duration_since(empty_since) >= self.empty_game_grace_period);

        match is_expired {
            true => GameFlow::End,
            false => GameFlow::Continue,
        }
    }

    fn admit_member(
        &mut self,
        admission: Result<JoinAdmission, RejectionKind>,
        endpoint: ConnectionEndpoint,
        reply: oneshot::Sender<Result<MemberId, RejectionKind>>,
        now: Instant,
    ) {
        let admission: JoinAdmission = match admission {
            Ok(admission) => admission,
            Err(rejection) => {
                let _ = reply.send(Err(rejection));

                return;
            }
        };

        let member_id: MemberId = admission.member_id;
        let join_accepted_frame: Bytes = connection_model::encode_frame(&MessageSerialOut::JoinAccepted {
            game_id: self.game_id.0,
            member_id: member_id.0,
        });

        self.recipients.add(member_id, endpoint, self.state.tick);
        self.recipients.send_to_member(member_id, join_accepted_frame, now);
        self.recipients.send_to_member(member_id, self.encode_snapshot(), now);

        for member_event in admission.member_events {
            self.bundle_builder.queue_member_event(member_event);
        }

        self.empty_since = None;
        let _ = reply.send(Ok(member_id));

        log::info!(
            "member admitted; [game_id={} member_id={}]",
            self.game_id.0,
            member_id.0
        );
    }

    fn respawn_member(
        &mut self,
        member_id: MemberId,
        loadout: Loadout,
        team: TeamChoiceKind,
    ) -> Result<(), RejectionKind> {
        if !self.recipients.contains(member_id) {
            return Err(RejectionKind::NotInGame);
        }

        let member_event: MemberEvent =
            membership::admit_respawn(&self.state, &mut self.pending_admissions, member_id, loadout, team)?;
        self.bundle_builder.queue_member_event(member_event);

        Ok(())
    }

    fn remove_member(&mut self, member_id: MemberId) {
        let removed_endpoint: Option<ConnectionEndpoint> = self.recipients.remove(member_id);

        if removed_endpoint.is_some() {
            self.queue_departure(member_id);
        }
    }

    /// `Left` in the next bundle.
    fn queue_departure(&mut self, member_id: MemberId) {
        self.bundle_builder.queue_member_event(MemberEvent::Left { member_id });
        self.bundle_builder.forget_member(member_id);
    }

    fn send_tick_frames(&mut self, built_bundle: BuiltBundle, now: Instant) {
        let tick: Tick = self.state.tick;
        let bundle_serial_out: InputBundleSerialOut = InputBundleSerialOut::from(&built_bundle.bundle);

        if let Some(replay_capture) = &mut self.replay_capture {
            replay_capture.record_bundle(bundle_serial_out.clone());
        }

        let bundle_frame: Bytes = connection_model::encode_frame(&MessageSerialOut::InputBundle(bundle_serial_out));
        self.recipients.broadcast(&bundle_frame, now);

        for cursor_correction in built_bundle.cursor_corrections {
            self.send_cursor_correction(cursor_correction, now);
        }

        if tick.0 % CHECKSUM_INTERVAL_TICKS == 0 {
            let checksum_frame: Bytes = connection_model::encode_frame(&MessageSerialOut::StateChecksum {
                tick: tick.0,
                checksum: state_checksum::get_state_checksum(&self.state),
            });
            self.recipients.broadcast(&checksum_frame, now);
        }

        self.send_due_snapshots(tick, now);
    }

    fn send_cursor_correction(&mut self, cursor_correction: CursorCorrection, now: Instant) {
        let correction_frame: Bytes = connection_model::encode_frame(&MessageSerialOut::CursorCorrected {
            client_tick: cursor_correction.client_tick.0,
            cursor: SubpixelPointSerial::from(&cursor_correction.cursor),
        });

        self.recipients.send_to_member(cursor_correction.member_id, correction_frame, now);
    }

    fn send_due_snapshots(&mut self, tick: Tick, now: Instant) {
        let snapshot_member_ids: Vec<MemberId> = self.recipients.get_snapshot_member_ids(tick);

        if snapshot_member_ids.is_empty() {
            return;
        }

        let snapshot_frame: Bytes = self.encode_snapshot();

        for member_id in snapshot_member_ids {
            self.recipients.send_snapshot(member_id, snapshot_frame.clone(), tick, now);
        }
    }

    fn encode_snapshot(&self) -> Bytes {
        connection_model::encode_frame(&MessageSerialOut::Snapshot(GameSnapshotSerialOut {
            game_id: self.game_id.0,
            state: GameStateSerialOut::from(&self.state),
        }))
    }

    fn handle_simulation_events(&mut self, simulation_events: &[SimulationEvent]) {
        for simulation_event in simulation_events {
            match simulation_event {
                SimulationEvent::OrganismSpawned { member_id, cursor } => {
                    self.bundle_builder.record_spawn(*member_id, *cursor, self.state.tick);
                }
                SimulationEvent::MemberLeft { member_id } => {
                    log::info!("member left; [game_id={} member_id={}]", self.game_id.0, member_id.0);
                }
                SimulationEvent::RoundPhaseChanged { phase } => {
                    log::debug!("round phase changed; [game_id={} phase={phase:?}]", self.game_id.0);
                }
                SimulationEvent::MemberJoined { .. }
                | SimulationEvent::SpawnRejected { .. }
                | SimulationEvent::OrganismDied { .. }
                | SimulationEvent::RoundWon { .. }
                | SimulationEvent::EffectApplied { .. } => {}
            }
        }
    }

    fn update_summary(&mut self) {
        let game_summary: GameSummary = GameSummary::from_game_state(self.game_id, &self.state, self.is_secured);

        self.summary.send_if_modified(|current_summary| {
            if *current_summary == game_summary {
                return false;
            }

            *current_summary = game_summary;

            true
        });
    }

    fn update_empty_since(&mut self, now: Instant) {
        if !self.state.members.is_empty() {
            self.empty_since = None;

            return;
        }

        self.empty_since.get_or_insert(now);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameFlow {
    Continue,
    End,
}

/// Counts consecutive ticks fired at least one period after they were due.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TickLag {
    late_tick_count: u32,
}

impl TickLag {
    pub fn observe(&mut self, due_at: Instant, fired_at: Instant, tick_period: Duration) -> TickLagKind {
        let is_late: bool = fired_at.saturating_duration_since(due_at) >= tick_period;

        if !is_late {
            self.late_tick_count = 0;

            return TickLagKind::OnTime;
        }

        self.late_tick_count += 1;

        if self.late_tick_count > TICK_LAG_RESET_THRESHOLD {
            self.late_tick_count = 0;

            return TickLagKind::ResetDue;
        }

        TickLagKind::Late
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickLagKind {
    OnTime,
    Late,
    /// The tick count, not wall time, is the game clock.
    ResetDue,
}

enum GameWakeKind {
    Tick(Instant),
    Request(Option<GameRequest>),
    Input(Option<GameInput>),
}

/// Runs until the game is empty past its grace period or its step fails; queued requests are then answered
/// `GameNotFound`.
pub async fn run_game_task(
    mut game_task: GameTask,
    mut requests: mpsc::Receiver<GameRequest>,
    mut inputs: mpsc::Receiver<GameInput>,
    tick_period: Duration,
) {
    let mut interval: Interval = tokio::time::interval_at(tokio::time::Instant::now() + tick_period, tick_period);
    interval.set_missed_tick_behavior(MissedTickBehavior::Burst);
    let mut tick_lag: TickLag = TickLag::default();

    loop {
        let wake: GameWakeKind = tokio::select! {
            due_at = interval.tick() => GameWakeKind::Tick(due_at.into_std()),
            request = requests.recv() => GameWakeKind::Request(request),
            input = inputs.recv() => GameWakeKind::Input(input),
        };

        let flow: GameFlow = match wake {
            GameWakeKind::Tick(due_at) => {
                let now: Instant = Instant::now();
                let tick_lag_kind: TickLagKind = tick_lag.observe(due_at, now, tick_period);

                if tick_lag_kind == TickLagKind::ResetDue {
                    log::warn!("game fell behind; [game_id={}]", game_task.game_id.0);
                    interval.reset();
                }

                run_tick_and_drain(&mut game_task, &mut requests, &mut inputs, now)
            }
            GameWakeKind::Request(Some(request)) => {
                game_task.handle_request(request, Instant::now());

                GameFlow::Continue
            }
            GameWakeKind::Input(Some(game_input)) => {
                game_task.handle_input(game_input);

                GameFlow::Continue
            }
            GameWakeKind::Request(None) | GameWakeKind::Input(None) => GameFlow::End,
        };

        if flow == GameFlow::End {
            break;
        }
    }

    close_game(&mut requests, &mut inputs);

    log::info!(
        "game ended; [game_id={} tick={} stale_inputs={}]",
        game_task.game_id.0,
        game_task.state.tick.0,
        game_task.stale_input_count,
    );
}

/// A tick burst cannot starve the queued requests and inputs.
fn run_tick_and_drain(
    game_task: &mut GameTask,
    requests: &mut mpsc::Receiver<GameRequest>,
    inputs: &mut mpsc::Receiver<GameInput>,
    now: Instant,
) -> GameFlow {
    let tick_flow: GameFlow = game_task.run_tick(now);

    if tick_flow == GameFlow::End {
        return GameFlow::End;
    }

    for _ in 0..COMMAND_DRAIN_LIMIT_PER_TICK {
        let Ok(request) = requests.try_recv() else {
            break;
        };

        game_task.handle_request(request, now);
    }

    for _ in 0..COMMAND_DRAIN_LIMIT_PER_TICK {
        let Ok(game_input) = inputs.try_recv() else {
            break;
        };

        game_task.handle_input(game_input);
    }

    game_task.get_flow(now)
}

fn close_game(requests: &mut mpsc::Receiver<GameRequest>, inputs: &mut mpsc::Receiver<GameInput>) {
    requests.close();
    inputs.close();

    while let Ok(request) = requests.try_recv() {
        answer_closed_game(request);
    }
}

fn answer_closed_game(request: GameRequest) {
    match request {
        GameRequest::Join { reply, .. } | GameRequest::Spectate { reply, .. } => {
            let _ = reply.send(Err(RejectionKind::GameNotFound));
        }
        GameRequest::Respawn { reply, .. } => {
            let _ = reply.send(Err(RejectionKind::GameNotFound));
        }
        GameRequest::Leave { reply, .. } => {
            let _ = reply.send(());
        }
        GameRequest::UpdateAppearance { .. }
        | GameRequest::ConnectionClosed { .. }
        | GameRequest::RequestSnapshot { .. } => {}
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib game::game_task`

Expected: `test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 82 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/game/mod.rs server/src/game/game_task.rs
git commit -m "game task"
```

### Task 20: Game supervisor

**Files:**
- Create: `server/src/game/game_supervisor.rs`
- Modify: `server/src/game/mod.rs`
- Test: inline `mod tests` in `server/src/game/game_supervisor.rs`

`launch_game` builds the state, the channels and the summary watch, starts the replay capture when configured, registers the `GameHandle`, and spawns the game task and its supervisor on `GAME_RUNTIME`. The supervisor is the only code which unregisters a game, after a normal end or a panic (server.md, empty-game removal and failure isolation).

- [ ] **Step 1: Write the failing test**

Create `server/src/game/game_supervisor.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::GameModeKind;
    use std::sync::atomic::Ordering;

    fn wait_for_removal(registries: ServerRegistries) {
        game_runtime::GAME_RUNTIME.block_on(async {
            for _ in 0..500 {
                if registries.games.is_empty() {
                    break;
                }

                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });
    }

    fn assert_unregistered(registries: ServerRegistries) {
        assert!(registries.games.is_empty());
        assert!(registries.game_titles.is_empty());
        assert_eq!(registries.game_count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn launch_game_registers_the_game_until_it_ends_empty() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let reservation: GameReservation = game_registry::reserve_game(registries, "Fixture game").unwrap();
        let game_config: GameTaskConfig = GameTaskConfig {
            tick_period: Duration::from_millis(5),
            empty_game_grace_period: Duration::from_millis(50),
            replay_capture_directory: None,
        };

        let game_handle: GameHandle = launch_game(
            registries,
            &game_config,
            &reservation,
            test_fixture::create_settings(GameModeKind::FreeForAll, 8),
            1,
            Some(String::from("hash")),
        );

        assert!(game_registry::get_game(registries, reservation.game_id).is_some());
        assert!(game_handle.summary.borrow().secured);

        wait_for_removal(registries);

        assert_unregistered(registries);
    }

    #[test]
    fn supervise_game_unregisters_a_game_whose_task_panicked() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let reservation: GameReservation = game_registry::reserve_game(registries, "Fixture game").unwrap();
        game_registry::insert_game(
            registries,
            reservation.game_id,
            test_fixture::create_game_handle(
                reservation.game_id,
                test_fixture::create_settings(GameModeKind::FreeForAll, 8),
            ),
        );

        let task_handle: JoinHandle<()> = game_runtime::GAME_RUNTIME.spawn(async { panic!("step failed") });
        game_runtime::GAME_RUNTIME.block_on(supervise_game(registries, reservation, task_handle));

        assert_unregistered(registries);
    }
}
```

Replace the whole contents of `server/src/game/mod.rs` with:

```rust
pub mod bundle_builder;
pub mod cursor_clamp;
pub mod game_handle;
pub mod game_registry;
pub mod game_runtime;
pub mod game_supervisor;
pub mod game_task;
pub mod membership;
pub mod pending_admissions;
pub mod recipients;
pub mod replay_capture;
// Fixtures shared by the unit tests of several modules; absent from every non-test build.
#[cfg(test)]
pub mod test_fixture;

pub use bundle_builder::*;
pub use cursor_clamp::*;
pub use game_handle::*;
pub use game_registry::*;
pub use game_runtime::*;
pub use game_supervisor::*;
pub use game_task::*;
pub use membership::*;
pub use pending_admissions::*;
pub use recipients::*;
pub use replay_capture::*;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib game::game_supervisor`

Expected: compile failure, first line ``error[E0425]: cannot find type `ServerRegistries` in this scope``. The passing run later prints the panic message of the deliberately panicking task; that is expected.

- [ ] **Step 3: Implement**

In `server/src/game/game_supervisor.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::task::{JoinError, JoinHandle};

use shared::game::{GameSettings, GameState, GameSummary};
use shared::protocol::{GameSettingsSerialOut, ReplayHeaderSerialOut};

use crate::config::ServerConfig;
use crate::game::{GameHandle, GameInput, GameRequest, GameReservation, GameTask, ReplayCapture};
use crate::game::{game_handle, game_registry, game_runtime, game_task};
use crate::registries::ServerRegistries;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameTaskConfig {
    pub tick_period: Duration,
    pub empty_game_grace_period: Duration,
    pub replay_capture_directory: Option<PathBuf>,
}

impl GameTaskConfig {
    pub fn from_server_config(config: &ServerConfig) -> GameTaskConfig {
        GameTaskConfig {
            tick_period: config.tick_period,
            empty_game_grace_period: config.empty_game_grace_period,
            replay_capture_directory: config.replay_capture_directory.clone(),
        }
    }
}

/// The game stays registered until its task ends.
pub fn launch_game(
    registries: ServerRegistries,
    game_config: &GameTaskConfig,
    reservation: &GameReservation,
    settings: GameSettings,
    seed: u64,
    password_hash: Option<String>,
) -> GameHandle {
    let state: GameState = GameState::new(settings.clone(), seed);
    let is_secured: bool = password_hash.is_some();
    let (request_sender, request_receiver): (mpsc::Sender<GameRequest>, mpsc::Receiver<GameRequest>) =
        mpsc::channel(game_handle::GAME_REQUEST_CHANNEL_CAPACITY);
    let (input_sender, input_receiver): (mpsc::Sender<GameInput>, mpsc::Receiver<GameInput>) =
        mpsc::channel(game_handle::GAME_INPUT_CHANNEL_CAPACITY);
    let (summary_sender, summary_receiver): (watch::Sender<GameSummary>, watch::Receiver<GameSummary>) =
        watch::channel(GameSummary::from_game_state(reservation.game_id, &state, is_secured));
    let replay_capture: Option<ReplayCapture> = start_replay_capture(game_config, reservation, &settings, seed);

    let game_task: GameTask = GameTask::new(
        reservation.game_id,
        state,
        is_secured,
        summary_sender,
        replay_capture,
        game_config.empty_game_grace_period,
    );
    let game_handle: GameHandle = GameHandle {
        requests: request_sender,
        inputs: input_sender,
        summary: summary_receiver,
        password_hash,
        settings: settings.clone(),
    };

    game_registry::insert_game(registries, reservation.game_id, game_handle.clone());

    let task_handle: JoinHandle<()> = game_runtime::GAME_RUNTIME.spawn(game_task::run_game_task(
        game_task,
        request_receiver,
        input_receiver,
        game_config.tick_period,
    ));
    game_runtime::GAME_RUNTIME.spawn(supervise_game(registries, reservation.clone(), task_handle));

    log::info!(
        "game created; [game_id={} mode={}]",
        reservation.game_id.0,
        settings.mode.code()
    );

    game_handle
}

fn start_replay_capture(
    game_config: &GameTaskConfig,
    reservation: &GameReservation,
    settings: &GameSettings,
    seed: u64,
) -> Option<ReplayCapture> {
    let directory: &PathBuf = game_config.replay_capture_directory.as_ref()?;
    let header_serial_out: ReplayHeaderSerialOut = ReplayHeaderSerialOut {
        settings: GameSettingsSerialOut::from(settings),
        seed,
    };
    let (replay_capture, _writer_handle): (ReplayCapture, JoinHandle<()>) =
        ReplayCapture::start(directory, reservation.game_id, header_serial_out);

    Some(replay_capture)
}

/// The only code which unregisters a game, however its task ended.
async fn supervise_game(registries: ServerRegistries, reservation: GameReservation, task_handle: JoinHandle<()>) {
    let task_result: Result<(), JoinError> = task_handle.await;

    if let Err(error) = task_result {
        log::error!("game task failed; [game_id={} error={error}]", reservation.game_id.0);
    }

    game_registry::remove_game(registries, reservation.game_id, &reservation.title);

    log::info!("game removed; [game_id={}]", reservation.game_id.0);
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib game::game_supervisor`

Expected: `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 98 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/game/mod.rs server/src/game/game_supervisor.rs
git commit -m "game supervisor"
```

### Task 21: Game list

**Files:**
- Create: `server/src/lobby/game_list.rs`
- Modify: `server/src/lobby/mod.rs`
- Test: inline `mod tests` in `server/src/lobby/game_list.rs`

The lobby's `GameList`: every registered game's summary, read from its `GameHandle.summary` watch, in ascending game id, plus the online client count (protocol.md, lobby).

- [ ] **Step 1: Write the failing test**

Create `server/src/lobby/game_list.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::{GameId, GameModeKind, GameSettings};
    use shared::protocol;

    #[test]
    fn get_game_list_lists_every_game_in_game_id_order_with_the_client_count() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let settings: GameSettings = test_fixture::create_settings(GameModeKind::FreeForAll, 8);

        for game_id in [GameId(9), GameId(2), GameId(5)] {
            registries.games.insert(game_id, test_fixture::create_game_handle(game_id, settings.clone()));
        }

        registries.connection_count.store(3, Ordering::SeqCst);
        let game_list: GameList = get_game_list(registries);

        assert_eq!(
            game_list.game_summaries.iter().map(|game_summary| game_summary.game_id).collect::<Vec<GameId>>(),
            vec![GameId(2), GameId(5), GameId(9)],
        );
        assert_eq!(game_list.online_client_count, 3);
    }

    #[test]
    fn encode_frame_encodes_a_game_list_message() {
        let game_list: GameList = GameList {
            game_summaries: Vec::new(),
            online_client_count: 4,
        };

        assert_eq!(
            protocol::decode_message_out(&game_list.encode_frame()).unwrap(),
            MessageSerialOut::GameList {
                game_summaries: Vec::new(),
                online_client_count: 4,
            },
        );
    }
}
```

Replace the whole contents of `server/src/lobby/mod.rs` with:

```rust
pub mod game_list;
pub mod lobby_broadcast;

pub use game_list::*;
pub use lobby_broadcast::*;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib lobby::game_list`

Expected: compile failure, first line ``error[E0425]: cannot find type `ServerRegistries` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/lobby/game_list.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::sync::atomic::Ordering;

use bytes::Bytes;

use shared::game::GameSummary;
use shared::protocol::{GameSummarySerialOut, MessageSerialOut};

use crate::registries::ServerRegistries;
use crate::socket::connection_model;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameList {
    /// In ascending game id.
    pub game_summaries: Vec<GameSummary>,
    pub online_client_count: u32,
}

impl GameList {
    pub fn encode_frame(&self) -> Bytes {
        connection_model::encode_frame(&MessageSerialOut::GameList {
            game_summaries: self.game_summaries.iter().map(GameSummarySerialOut::from).collect(),
            online_client_count: self.online_client_count,
        })
    }
}

pub fn get_game_list(registries: ServerRegistries) -> GameList {
    let mut game_summaries: Vec<GameSummary> =
        registries.games.iter().map(|entry| entry.value().summary.borrow().clone()).collect();
    game_summaries.sort_by_key(|game_summary| game_summary.game_id);

    GameList {
        game_summaries,
        online_client_count: registries.connection_count.load(Ordering::SeqCst),
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib lobby::game_list`

Expected: `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 100 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/lobby/mod.rs server/src/lobby/game_list.rs
git commit -m "game list"
```

### Task 22: Lobby broadcast

**Files:**
- Modify: `server/src/lobby/lobby_broadcast.rs`
- Test: inline `mod tests` in `server/src/lobby/lobby_broadcast.rs`

protocol.md (lobby): `SubscribeGameList` sends the current list at once; the lobby task sends `GameList` at most once per second, only when the revision changed. A full subscriber queue misses that list only; a closed one is removed.

- [ ] **Step 1: Write the failing test**

Append to the end of `server/src/lobby/lobby_broadcast.rs`, after one blank line:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use shared::game::{GameId, GameModeKind, GameState, GameSummary};
    use shared::protocol;
    use shared::protocol::MessageSerialOut;

    fn create_game_list(player_count: u8, online_client_count: u32) -> GameList {
        let state: GameState = test_fixture::create_state(GameModeKind::FreeForAll, 8);
        let mut game_summary: GameSummary = GameSummary::from_game_state(GameId(1), &state, false);
        game_summary.player_count = player_count;

        GameList {
            game_summaries: vec![game_summary],
            online_client_count,
        }
    }

    #[test]
    fn observe_changes_the_revision_on_a_summary_or_client_count_change_only() {
        let mut lobby_revision: LobbyRevision = LobbyRevision::new();

        lobby_revision.observe(create_game_list(0, 1));
        lobby_revision.observe(create_game_list(0, 1));
        lobby_revision.observe(create_game_list(1, 1));
        lobby_revision.observe(create_game_list(1, 2));

        assert_eq!(lobby_revision.revision(), 3);
    }

    #[test]
    fn take_due_list_gives_a_changed_list_at_most_once_per_interval() {
        let mut lobby_revision: LobbyRevision = LobbyRevision::new();
        let start: Instant = Instant::now();
        lobby_revision.observe(create_game_list(0, 1));

        assert_eq!(lobby_revision.take_due_list(start), Some(&create_game_list(0, 1)));
        assert_eq!(
            lobby_revision.take_due_list(start + LOBBY_BROADCAST_MINIMUM_INTERVAL),
            None
        );

        lobby_revision.observe(create_game_list(1, 1));

        assert_eq!(lobby_revision.take_due_list(start + Duration::from_millis(999)), None);
        assert_eq!(
            lobby_revision.take_due_list(start + LOBBY_BROADCAST_MINIMUM_INTERVAL),
            Some(&create_game_list(1, 1)),
        );
    }

    #[test]
    fn subscribe_sends_the_current_list_at_once() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let (lobby_sender, mut lobby_receiver): (mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) =
            mpsc::channel(LOBBY_CHANNEL_CAPACITY);

        subscribe(registries, ConnectionId(1), lobby_sender);

        assert!(matches!(
            protocol::decode_message_out(&lobby_receiver.try_recv().unwrap()).unwrap(),
            MessageSerialOut::GameList { .. },
        ));
        assert!(registries.lobby_subscribers.contains_key(&ConnectionId(1)));

        unsubscribe(registries, ConnectionId(1));

        assert!(registries.lobby_subscribers.is_empty());
    }

    #[test]
    fn broadcast_game_list_skips_a_full_subscriber_and_removes_a_closed_one() {
        let registries: ServerRegistries = ServerRegistries::leaked();
        let (full_sender, _full_receiver): (mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) = mpsc::channel(1);
        let (closed_sender, closed_receiver): (mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) = mpsc::channel(1);
        full_sender.try_send(Bytes::from_static(b"older list")).unwrap();
        drop(closed_receiver);
        registries.lobby_subscribers.insert(ConnectionId(1), full_sender);
        registries.lobby_subscribers.insert(ConnectionId(2), closed_sender);

        broadcast_game_list(registries, &Bytes::from_static(b"list"));

        assert!(registries.lobby_subscribers.contains_key(&ConnectionId(1)));
        assert!(!registries.lobby_subscribers.contains_key(&ConnectionId(2)));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib lobby::lobby_broadcast`

Expected: compile failure, first line ``error[E0425]: cannot find type `GameList` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/lobby/lobby_broadcast.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::time::{Interval, MissedTickBehavior};

use crate::lobby::GameList;
use crate::lobby::game_list;
use crate::password::password_failure_limit;
use crate::registries::ServerRegistries;
use crate::socket::ConnectionId;

pub const LOBBY_CHANNEL_CAPACITY: usize = 16;
pub const LOBBY_BROADCAST_MINIMUM_INTERVAL: Duration = Duration::from_secs(1);
const LOBBY_POLL_PERIOD: Duration = Duration::from_millis(250);

pub static LOBBY_SUBSCRIBERS: LazyLock<DashMap<ConnectionId, mpsc::Sender<Bytes>>> = LazyLock::new(DashMap::new);

/// The revision changes whenever a summary or the online client count does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LobbyRevision {
    revision: u64,
    sent_revision: u64,
    last_game_list: Option<GameList>,
    last_sent_at: Option<Instant>,
}

impl LobbyRevision {
    pub fn new() -> LobbyRevision {
        LobbyRevision::default()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn observe(&mut self, game_list: GameList) {
        if self.last_game_list.as_ref() == Some(&game_list) {
            return;
        }

        self.revision += 1;
        self.last_game_list = Some(game_list);
    }

    /// The latest list, when its revision is unsent and the minimum interval has passed.
    pub fn take_due_list(&mut self, now: Instant) -> Option<&GameList> {
        let is_interval_passed: bool = self
            .last_sent_at
            .is_none_or(|last_sent_at| now.saturating_duration_since(last_sent_at) >= LOBBY_BROADCAST_MINIMUM_INTERVAL);

        if self.revision == self.sent_revision || !is_interval_passed {
            return None;
        }

        self.sent_revision = self.revision;
        self.last_sent_at = Some(now);

        self.last_game_list.as_ref()
    }
}

/// Sends the current list at once.
pub fn subscribe(registries: ServerRegistries, connection_id: ConnectionId, lobby_sender: mpsc::Sender<Bytes>) {
    let game_list_frame: Bytes = game_list::get_game_list(registries).encode_frame();
    let _ = lobby_sender.try_send(game_list_frame);
    registries.lobby_subscribers.insert(connection_id, lobby_sender);
}

pub fn unsubscribe(registries: ServerRegistries, connection_id: ConnectionId) {
    registries.lobby_subscribers.remove(&connection_id);
}

pub async fn run_lobby_task(registries: ServerRegistries) {
    let mut interval: Interval = tokio::time::interval(LOBBY_POLL_PERIOD);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut lobby_revision: LobbyRevision = LobbyRevision::new();

    loop {
        interval.tick().await;
        let now: Instant = Instant::now();

        password_failure_limit::forget_expired_failures(registries.password_failures_per_ip, now);

        lobby_revision.observe(game_list::get_game_list(registries));
        let due_game_list: Option<&GameList> = lobby_revision.take_due_list(now);

        if let Some(game_list) = due_game_list {
            broadcast_game_list(registries, &game_list.encode_frame());
        }
    }
}

/// A full subscriber misses this list only; a closed one is removed.
fn broadcast_game_list(registries: ServerRegistries, game_list_frame: &Bytes) {
    registries.lobby_subscribers.retain(|_, lobby_sender| {
        let send_result: Result<(), TrySendError<Bytes>> = lobby_sender.try_send(game_list_frame.clone());

        !matches!(send_result, Err(TrySendError::Closed(_)))
    });
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib lobby::lobby_broadcast`

Expected: `test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 102 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/lobby/lobby_broadcast.rs
git commit -m "lobby broadcast"
```

### Task 23: Static site and health

**Files:**
- Create: `server/src/site/mod.rs`
- Create: `server/src/site/site_api.rs`
- Modify: `server/src/lib.rs`
- Test: inline `mod tests` in `server/src/site/site_api.rs`

server.md (production static serving) and protocol.md (HTTP endpoints): `actix_files::Files::new("/", site_root).index_file("index.html")` when a site root is configured, `/pkg/*` immutable for a year, `index.html` and `game-server.json` `no-cache`; `GET /health` answers `ok` with or without a site.

- [ ] **Step 1: Write the failing test**

Create `server/src/site/site_api.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::test_fixture;
    use actix_web::http::StatusCode;
    use actix_web::{App, test};
    use std::path::PathBuf;

    fn create_site_root(name: &str) -> PathBuf {
        let site_root: PathBuf = test_fixture::create_temporary_directory(name);
        std::fs::create_dir_all(site_root.join("pkg")).unwrap();
        std::fs::write(site_root.join("index.html"), "<!doctype html>").unwrap();
        std::fs::write(site_root.join("game-server.json"), "{}").unwrap();
        std::fs::write(site_root.join("pkg/bacter-0a1b.wasm"), "wasm").unwrap();
        std::fs::write(site_root.join("favicon.ico"), "icon").unwrap();

        site_root
    }

    /// The status, cache control header and body of each path.
    async fn get_responses(site_root: Option<&Path>, paths: &[&str]) -> Vec<(StatusCode, Option<String>, String)> {
        let service = test::init_service(App::new().configure(|service_config| {
            configurer(service_config, site_root);
        }))
        .await;
        let mut responses: Vec<(StatusCode, Option<String>, String)> = Vec::new();

        for path in paths {
            let response: ServiceResponse =
                test::call_service(&service, test::TestRequest::get().uri(path).to_request()).await;
            let status: StatusCode = response.status();
            let cache_control: Option<String> =
                response.headers().get(header::CACHE_CONTROL).map(|value| String::from(value.to_str().unwrap()));
            let body: String = String::from_utf8(test::read_body(response).await.to_vec()).unwrap();

            responses.push((status, cache_control, body));
        }

        responses
    }

    #[actix_web::test]
    async fn configurer_serves_the_site_with_its_cache_headers() {
        let site_root: PathBuf = create_site_root("site-cache-headers");
        let no_cache: Option<String> = Some(String::from(NO_CACHE_CONTROL));

        let responses: Vec<(StatusCode, Option<String>, String)> = get_responses(
            Some(&site_root),
            &["/", "/game-server.json", "/pkg/bacter-0a1b.wasm", "/favicon.ico"],
        )
        .await;

        assert_eq!(
            responses,
            vec![
                (StatusCode::OK, no_cache.clone(), String::from("<!doctype html>")),
                (StatusCode::OK, no_cache, String::from("{}")),
                (
                    StatusCode::OK,
                    Some(String::from(IMMUTABLE_CACHE_CONTROL)),
                    String::from("wasm")
                ),
                (StatusCode::OK, None, String::from("icon")),
            ],
        );
    }

    #[actix_web::test]
    async fn configurer_answers_health_with_or_without_a_site() {
        let site_root: PathBuf = create_site_root("site-health");
        let health_response: (StatusCode, Option<String>, String) = (StatusCode::OK, None, String::from("ok"));

        assert_eq!(
            get_responses(Some(&site_root), &["/health"]).await,
            vec![health_response.clone()]
        );
        assert_eq!(get_responses(None, &["/health"]).await, vec![health_response]);
    }

    #[actix_web::test]
    async fn configurer_serves_no_files_without_a_site_root() {
        let responses: Vec<(StatusCode, Option<String>, String)> = get_responses(None, &["/"]).await;

        assert_eq!(responses[0].0, StatusCode::NOT_FOUND);
    }
}
```

Create `server/src/site/mod.rs`:

```rust
pub mod site_api;

pub use site_api::*;
```

Replace the whole contents of `server/src/lib.rs` with:

```rust
//! The game server.

pub mod config;
pub mod game;
pub mod lobby;
pub mod password;
pub mod registries;
pub mod site;
pub mod socket;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib site::site_api`

Expected: compile failure, first line ``error[E0425]: cannot find type `Path` in this scope``.

- [ ] **Step 3: Implement**

In `server/src/site/site_api.rs`, replace everything above `#[cfg(test)]` with:

```rust
use std::path::Path;

use actix_files::Files;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::header;
use actix_web::http::header::HeaderValue;
use actix_web::middleware::Next;
use actix_web::{HttpResponse, middleware, web};

const INDEX_FILE_NAME: &str = "index.html";
const IMMUTABLE_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
const NO_CACHE_CONTROL: &str = "no-cache";

/// No files without a `site_root`.
pub fn configurer(service_config: &mut web::ServiceConfig, site_root: Option<&Path>) {
    service_config.route("/health", web::get().to(get_health));

    let Some(site_root) = site_root else {
        return;
    };

    service_config.service(
        web::scope("")
            .wrap(middleware::from_fn(set_cache_control))
            .service(Files::new("/", site_root).index_file(INDEX_FILE_NAME)),
    );
}

async fn get_health() -> HttpResponse {
    HttpResponse::Ok().content_type("text/plain; charset=utf-8").body("ok")
}

async fn set_cache_control(
    request: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let cache_control: Option<&'static str> = get_cache_control(request.path());
    let mut response: ServiceResponse<_> = next.call(request).await?;

    if let Some(cache_control) = cache_control {
        response.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static(cache_control));
    }

    Ok(response)
}

/// Hashed `/pkg/` files never change; the entry points must be revalidated.
fn get_cache_control(path: &str) -> Option<&'static str> {
    if path.starts_with("/pkg/") {
        return Some(IMMUTABLE_CACHE_CONTROL);
    }

    match path {
        "/" | "/index.html" | "/game-server.json" => Some(NO_CACHE_CONTROL),
        _ => None,
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p server --lib site::site_api`

Expected: `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 106 filtered out`.

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/lib.rs server/src/site/mod.rs server/src/site/site_api.rs
git commit -m "static site and health"
```

### Task 24: WebSocket endpoint

**Files:**
- Create: `server/src/shutdown/mod.rs`, `server/src/shutdown/shutdown.rs`
- Create: `server/src/socket/socket_writer.rs`
- Create: `server/src/socket/connection.rs`
- Create: `server/src/socket/request_dispatch.rs` (lobby requests only; Task 25 completes it)
- Create: `server/src/socket/socket_api.rs`
- Modify: `server/src/socket/mod.rs`
- Modify: `server/src/lib.rs`
- Create: `server/tests/helpers/mod.rs`, `server/tests/helpers/test_server.rs`
- Test: `server/tests/protocol_errors.rs`

The transport of server.md (process, tasks and channels) and protocol.md (transport, codec, version): the `Origin` check before the upgrade; the version and connection-limit closes right after it; the read task with its ping every 10 s, 30 s inbound timeout, 4096-byte frame limit, the close of a text, undecodable or oversize frame, its control watch and the run state watch; the writer task with its 5 s write timeout; `create_server`, which binds plain HTTP or rustls and starts the lobby task. The test helper binds `127.0.0.1:0` with `ServerRegistries::leaked()` inside the test's actix system and speaks to it over tokio-tungstenite; `TestPlayer` follows a game by stepping every bundle and checking every checksum.

- [ ] **Step 1: Write the failing test**

Create `server/tests/helpers/mod.rs`:

```rust
pub mod test_server;
```

Create `server/tests/helpers/test_server.rs`:

```rust
// Each test binary compiles this helper and uses only part of it.
#![allow(dead_code)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::client::{Request, Response};
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::http::header;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use server::CreatedServer;
use server::config::ServerConfig;
use server::registries::ServerRegistries;
use server::shutdown::ServerRunState;
use shared::ability::{FirstAbilityKind, Loadout, SecondAbilityKind, ThirdAbilityKind};
use shared::game;
use shared::game::{GameModeKind, GameState, InputBundle, Tick};
use shared::member::{Appearance, MemberId, OrganismColorKind, SkinKind};
use shared::organism::Organism;
use shared::protocol;
use shared::protocol::state_checksum;
use shared::protocol::{
    GameModeKindSerial, GameSettingsSerialIn, JoinerSerialIn, LoadoutSerial, MessageSerialIn, MessageSerialOut,
    TeamChoiceKindSerialIn, WorldShapeKindSerial,
};

pub const TEST_ORIGIN: &str = "http://127.0.0.1:3011";
/// For one awaited message.
pub const RECEIVE_TIMEOUT: Duration = Duration::from_secs(10);
/// For a sequence of messages, up to the longest test.
pub const PLAY_TIMEOUT: Duration = Duration::from_secs(120);

pub type TestSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct TestServer {
    pub address: SocketAddr,
    pub registries: ServerRegistries,
    pub run_state: watch::Sender<ServerRunState>,
}

pub struct TestClient {
    stream: TestSocket,
}

impl TestClient {
    pub async fn connect(test_server: &TestServer) -> TestClient {
        TestClient::try_connect(test_server, Some(TEST_ORIGIN), Some(protocol::PROTOCOL_VERSION))
            .await
            .expect("the test client connects")
    }

    pub async fn try_connect(
        test_server: &TestServer,
        origin: Option<&str>,
        protocol_version: Option<u16>,
    ) -> Result<TestClient, WebSocketError> {
        let query: String = protocol_version.map_or(String::new(), |version| format!("?protocol_version={version}"));
        let mut request: Request = format!("ws://{}/ws{query}", test_server.address).into_client_request()?;

        if let Some(origin) = origin {
            request.headers_mut().insert(header::ORIGIN, HeaderValue::from_str(origin).unwrap());
        }

        let (stream, _): (TestSocket, Response) = tokio_tungstenite::connect_async(request).await?;

        Ok(TestClient { stream })
    }

    pub fn split(self) -> (SplitSink<TestSocket, Message>, SplitStream<TestSocket>) {
        self.stream.split()
    }

    pub async fn send(&mut self, message_serial_in: &MessageSerialIn) {
        self.send_raw(Message::Binary(protocol::encode_message_in(message_serial_in).into())).await;
    }

    pub async fn send_raw(&mut self, message: Message) {
        self.stream.send(message).await.expect("the test client sends");
    }

    /// The next server message; fails on a close or after the receive timeout.
    pub async fn receive(&mut self) -> MessageSerialOut {
        let deadline: Instant = Instant::now() + RECEIVE_TIMEOUT;

        loop {
            match self.receive_frame(deadline).await {
                Message::Binary(frame) => {
                    return protocol::decode_message_out(&frame).expect("a server message decodes");
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                other_message => panic!("expected a server message, received {other_message:?}"),
            }
        }
    }

    pub async fn receive_until(&mut self, mut is_wanted: impl FnMut(&MessageSerialOut) -> bool) -> MessageSerialOut {
        let receiving = async {
            loop {
                let message_serial_out: MessageSerialOut = self.receive().await;

                if is_wanted(&message_serial_out) {
                    return message_serial_out;
                }
            }
        };

        tokio::time::timeout(PLAY_TIMEOUT, receiving)
            .await
            .expect("the wanted message arrives within the play timeout")
    }

    /// The close code and reason; game frames before the close are skipped.
    pub async fn receive_close(&mut self) -> (u16, String) {
        let deadline: Instant = Instant::now() + RECEIVE_TIMEOUT;

        loop {
            match self.receive_frame(deadline).await {
                Message::Close(Some(CloseFrame { code, reason })) => return (u16::from(code), reason.to_string()),
                Message::Close(None) => panic!("the server closed without a close frame"),
                _ => continue,
            }
        }
    }

    /// Pings would restart a per-frame timeout, so the deadline is the caller's.
    async fn receive_frame(&mut self, deadline: Instant) -> Message {
        let frame: Option<Result<Message, WebSocketError>> = tokio::time::timeout_at(deadline, self.stream.next())
            .await
            .expect("a frame arrives within the receive timeout");

        frame.expect("the connection is open").expect("the frame is valid")
    }
}

/// A client which follows its game by stepping every bundle and checking every checksum.
pub struct TestPlayer {
    pub client: TestClient,
    pub game_id: u32,
    pub member_id: MemberId,
    pub state: GameState,
    pub verified_checksum_count: u32,
}

impl TestPlayer {
    /// Reads `JoinAccepted` then the snapshot.
    pub async fn enter(mut client: TestClient) -> TestPlayer {
        let MessageSerialOut::JoinAccepted { game_id, member_id } = client.receive().await else {
            panic!("expected JoinAccepted");
        };
        let MessageSerialOut::Snapshot(snapshot_serial_out) = client.receive().await else {
            panic!("expected the snapshot after JoinAccepted");
        };

        assert_eq!(snapshot_serial_out.game_id, game_id);

        TestPlayer {
            client,
            game_id,
            member_id: MemberId(member_id),
            state: GameState::try_from(snapshot_serial_out.state).expect("the snapshot converts"),
            verified_checksum_count: 0,
        }
    }

    /// Applies one game message; returns the bundle when it was one.
    pub fn apply(&mut self, message_serial_out: &MessageSerialOut) -> Option<InputBundle> {
        match message_serial_out {
            MessageSerialOut::InputBundle(bundle_serial_out) => {
                let bundle: InputBundle =
                    InputBundle::try_from(bundle_serial_out.clone()).expect("the bundle converts");
                game::step(&mut self.state, &bundle).expect("the bundle steps");

                Some(bundle)
            }
            MessageSerialOut::StateChecksum { tick, checksum } => {
                assert_eq!(*tick, self.state.tick.0);
                assert_eq!(*checksum, state_checksum::get_state_checksum(&self.state));
                self.verified_checksum_count += 1;

                None
            }
            MessageSerialOut::Snapshot(snapshot_serial_out) => {
                self.state = GameState::try_from(snapshot_serial_out.state.clone()).expect("the snapshot converts");

                None
            }
            _ => None,
        }
    }

    /// Receives and applies messages until `is_done` holds for the player.
    pub async fn play_until(&mut self, mut is_done: impl FnMut(&TestPlayer) -> bool) {
        let playing = async {
            while !is_done(self) {
                let message_serial_out: MessageSerialOut = self.client.receive().await;
                self.apply(&message_serial_out);
            }
        };

        tokio::time::timeout(PLAY_TIMEOUT, playing)
            .await
            .expect("the player gets there within the play timeout");
    }

    /// Receives and applies messages until the next bundle, which it returns.
    pub async fn receive_bundle(&mut self) -> InputBundle {
        loop {
            let message_serial_out: MessageSerialOut = self.client.receive().await;
            let bundle: Option<InputBundle> = self.apply(&message_serial_out);

            if let Some(bundle) = bundle {
                return bundle;
            }
        }
    }

    pub fn get_own_organism(&self) -> Option<&Organism> {
        self.state.members.get(&self.member_id).and_then(|member| member.organism.as_ref())
    }

    pub fn applied_tick(&self) -> Tick {
        self.state.tick
    }
}

/// Binds a free port with fresh registries; must run inside an actix system.
pub fn start_test_server(customize_config: impl FnOnce(&mut ServerConfig)) -> TestServer {
    let mut config: ServerConfig = ServerConfig {
        bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port: 0,
        site_root: None,
        allowed_origins: vec![String::from(TEST_ORIGIN)],
        replay_capture_directory: None,
        tls_files: None,
        empty_game_grace_period: Duration::from_secs(10),
        tick_period: Duration::from_millis(70),
    };
    customize_config(&mut config);

    let registries: ServerRegistries = ServerRegistries::leaked();
    let created_server: CreatedServer = server::create_server(config, registries).expect("the test server binds");
    actix_web::rt::spawn(created_server.server);

    TestServer {
        address: created_server.local_addresses[0],
        registries,
        run_state: created_server.run_state,
    }
}

pub fn create_loadout_serial() -> LoadoutSerial {
    LoadoutSerial::from(&Loadout {
        appearance: Appearance {
            color: OrganismColorKind::Sky,
            skin: SkinKind::Grid,
        },
        first: FirstAbilityKind::Extend,
        second: SecondAbilityKind::Immortality,
        third: ThirdAbilityKind::Toxin,
    })
}

pub fn create_joiner_serial_in(screen_name: &str, team: TeamChoiceKindSerialIn) -> JoinerSerialIn {
    JoinerSerialIn {
        screen_name: String::from(screen_name),
        loadout: create_loadout_serial(),
        team,
    }
}

/// A square rectangle world; survival needs 2 players and skirmish has 2 teams.
pub fn create_settings_serial_in(
    title: &str,
    mode: GameModeKind,
    world_size_pixels: u32,
    player_cap: u8,
) -> GameSettingsSerialIn {
    GameSettingsSerialIn {
        title: String::from(title),
        mode: GameModeKindSerial::from(&mode),
        world_shape: WorldShapeKindSerial::Rectangle,
        world_size_pixels,
        player_minimum: (mode == GameModeKind::Survival).then_some(2),
        player_cap,
        team_count: (mode == GameModeKind::Skirmish).then_some(2),
        leaderboard_length: 10,
    }
}

pub fn create_game_message(title: &str, password: Option<&str>, screen_name: &str) -> MessageSerialIn {
    MessageSerialIn::CreateGame {
        settings: create_settings_serial_in(title, GameModeKind::FreeForAll, 800, 8),
        password: password.map(String::from),
        joiner: create_joiner_serial_in(screen_name, TeamChoiceKindSerialIn::Auto),
    }
}

pub fn join_game_message(game_id: u32, password: Option<&str>, screen_name: &str) -> MessageSerialIn {
    MessageSerialIn::JoinGame {
        game_id,
        password: password.map(String::from),
        joiner: create_joiner_serial_in(screen_name, TeamChoiceKindSerialIn::Auto),
    }
}

/// Connects, creates a free-for-all game and enters it.
pub async fn create_game(test_server: &TestServer, title: &str, screen_name: &str) -> TestPlayer {
    let mut client: TestClient = TestClient::connect(test_server).await;
    client.send(&create_game_message(title, None, screen_name)).await;

    TestPlayer::enter(client).await
}

/// Connects and joins `game_id`.
pub async fn join_game(test_server: &TestServer, game_id: u32, screen_name: &str) -> TestPlayer {
    let mut client: TestClient = TestClient::connect(test_server).await;
    client.send(&join_game_message(game_id, None, screen_name)).await;

    TestPlayer::enter(client).await
}
```

Create `server/tests/protocol_errors.rs`:

```rust
mod helpers;

use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message};

use helpers::test_server;
use helpers::test_server::{TestClient, TestServer};
use server::shutdown::ServerRunState;
use shared::protocol;
use shared::protocol::protocol_limits;
use shared::protocol::{MessageSerialIn, MessageSerialOut, RejectionKindSerialOut, RequestKindSerialOut};

fn rejection(request: RequestKindSerialOut, reason: RejectionKindSerialOut) -> MessageSerialOut {
    MessageSerialOut::RequestRejected { request, reason }
}

#[actix_web::test]
async fn open_socket_closes_a_connection_with_a_wrong_or_missing_protocol_version() {
    let test_server: TestServer = test_server::start_test_server(|_| {});

    for protocol_version in [Some(0), None] {
        let mut client: TestClient =
            TestClient::try_connect(&test_server, Some(test_server::TEST_ORIGIN), protocol_version)
                .await
                .unwrap();

        assert_eq!(
            client.receive_close().await,
            (4001, String::from("server protocol version 1"))
        );
    }
}

#[actix_web::test]
async fn open_socket_refuses_a_handshake_from_a_disallowed_or_missing_origin() {
    let test_server: TestServer = test_server::start_test_server(|_| {});

    for origin in [Some("http://evil.example"), None] {
        let connection_result: Result<TestClient, WebSocketError> =
            TestClient::try_connect(&test_server, origin, Some(protocol::PROTOCOL_VERSION)).await;

        assert!(matches!(connection_result, Err(WebSocketError::Http(response)) if response.status() == 403));
    }
}

#[actix_web::test]
async fn run_connection_closes_on_a_text_frame() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut client: TestClient = TestClient::connect(&test_server).await;

    client.send_raw(Message::text("hello")).await;

    assert_eq!(client.receive_close().await, (1008, String::from("text frame")));
}

#[actix_web::test]
async fn run_connection_closes_on_an_oversize_frame() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut client: TestClient = TestClient::connect(&test_server).await;

    client.send_raw(Message::binary(vec![0; protocol_limits::INBOUND_FRAME_BYTE_LIMIT + 1])).await;

    assert_eq!(client.receive_close().await, (1008, String::from("oversize")));
}

#[actix_web::test]
async fn run_connection_closes_only_the_connection_which_sent_garbage() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut garbage_client: TestClient = TestClient::connect(&test_server).await;
    let mut other_client: TestClient = TestClient::connect(&test_server).await;

    garbage_client.send_raw(Message::binary(vec![0xff; 32])).await;

    assert_eq!(garbage_client.receive_close().await, (1008, String::from("decode")));

    other_client.send(&MessageSerialIn::SubscribeGameList).await;

    assert!(matches!(
        other_client.receive().await,
        MessageSerialOut::GameList { .. }
    ));
}

#[actix_web::test]
async fn dispatch_answers_requests_beyond_the_rate_limit_with_rate_limited() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut client: TestClient = TestClient::connect(&test_server).await;

    for _ in 0..9 {
        client.send(&MessageSerialIn::SubscribeGameList).await;
    }

    let mut message_serial_outs: Vec<MessageSerialOut> = Vec::new();

    for _ in 0..9 {
        message_serial_outs.push(client.receive().await);
    }

    let game_list_count: usize = message_serial_outs
        .iter()
        .filter(|message_serial_out| matches!(message_serial_out, MessageSerialOut::GameList { .. }))
        .count();

    assert_eq!(game_list_count, 8);
    assert!(message_serial_outs.contains(&rejection(
        RequestKindSerialOut::SubscribeGameList,
        RejectionKindSerialOut::RateLimited,
    )));
}

#[actix_web::test]
async fn open_socket_closes_connections_beyond_the_per_ip_limit_with_server_full() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut clients: Vec<TestClient> = Vec::new();

    for _ in 0..protocol_limits::CONNECTION_LIMIT_PER_IP {
        let mut client: TestClient = TestClient::connect(&test_server).await;
        client.send(&MessageSerialIn::SubscribeGameList).await;
        client.receive().await;
        clients.push(client);
    }

    let mut extra_client: TestClient = TestClient::connect(&test_server).await;

    assert_eq!(extra_client.receive_close().await, (4006, String::from("server full")));
}

#[actix_web::test]
async fn run_connection_closes_every_connection_when_the_server_shuts_down() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut idle_client: TestClient = TestClient::connect(&test_server).await;
    let mut lobby_client: TestClient = TestClient::connect(&test_server).await;
    lobby_client.send(&MessageSerialIn::SubscribeGameList).await;
    lobby_client.receive().await;

    test_server.run_state.send(ServerRunState::ShuttingDown).unwrap();

    assert_eq!(
        idle_client.receive_close().await,
        (1001, String::from("server shutting down"))
    );
    assert_eq!(
        lobby_client.receive_close().await,
        (1001, String::from("server shutting down"))
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p server --lib --test protocol_errors`

Expected: compile failure, first line ``error[E0432]: unresolved import `server::CreatedServer` ``.

- [ ] **Step 3: Implement the shutdown state, the writer and the read task**

Create `server/src/shutdown/shutdown.rs`:

```rust
#[cfg(not(unix))] // unix targets also wait for SIGTERM
pub use other::wait_for_termination_signal;
#[cfg(unix)] // other targets have no SIGTERM
pub use unix::wait_for_termination_signal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerRunState {
    Running,
    ShuttingDown,
}

#[cfg(unix)] // other targets have no SIGTERM
mod unix {
    use std::io;

    use actix_web::rt::signal;
    use actix_web::rt::signal::unix::{Signal, SignalKind};

    /// Interrupt or terminate.
    pub async fn wait_for_termination_signal() {
        let terminate_signal: io::Result<Signal> = signal::unix::signal(SignalKind::terminate());
        let Ok(mut terminate_signal) = terminate_signal else {
            let _ = signal::ctrl_c().await;

            return;
        };

        tokio::select! {
            _ = signal::ctrl_c() => {}
            _ = terminate_signal.recv() => {}
        }
    }
}

#[cfg(not(unix))] // unix targets also wait for SIGTERM
mod other {
    use actix_web::rt::signal;

    pub async fn wait_for_termination_signal() {
        let _ = signal::ctrl_c().await;
    }
}
```

Create `server/src/shutdown/mod.rs`:

```rust
pub mod shutdown;

pub use shutdown::*;
```

Create `server/src/socket/socket_writer.rs`:

```rust
use std::time::Duration;

use actix_ws::{CloseCode, CloseReason, Closed, Session};
use bytes::Bytes;
use tokio::sync::mpsc;
use tokio::time::error::Elapsed;

use crate::socket::CloseCauseKind;

pub const SOCKET_WRITE_TIMEOUT: Duration = Duration::from_secs(5);
pub const WRITER_COMMAND_CHANNEL_CAPACITY: usize = 32;

pub enum WriterCommand {
    Frame(Bytes),
    AttachMembership(mpsc::Receiver<Bytes>),
    /// Drops the membership queue unread, so no frame of the old game follows `then_send`.
    DetachMembership {
        then_send: Option<Bytes>,
    },
    Ping,
    Pong(Bytes),
    Close(CloseCauseKind),
}

enum WriterEventKind {
    Command(Option<WriterCommand>),
    MembershipFrame(Option<Bytes>),
    LobbyFrame(Option<Bytes>),
}

enum WriterFlow {
    Continue,
    Close(CloseCauseKind),
    End,
}

/// The only owner of the session; a write which does not finish within the timeout drops it without a close frame.
pub async fn run_socket_writer(
    mut session: Session,
    mut commands: mpsc::Receiver<WriterCommand>,
    mut lobby_frames: mpsc::Receiver<Bytes>,
) {
    let mut membership_frames: Option<mpsc::Receiver<Bytes>> = None;
    let mut is_lobby_open: bool = true;

    loop {
        let event: WriterEventKind = tokio::select! {
            biased;
            command = commands.recv() => WriterEventKind::Command(command),
            frame = receive_membership_frame(&mut membership_frames) => WriterEventKind::MembershipFrame(frame),
            frame = lobby_frames.recv(), if is_lobby_open => WriterEventKind::LobbyFrame(frame),
        };

        let flow: WriterFlow = match event {
            WriterEventKind::Command(Some(command)) => {
                handle_command(&mut session, &mut membership_frames, command).await
            }
            WriterEventKind::Command(None) => WriterFlow::End,
            WriterEventKind::MembershipFrame(Some(frame)) | WriterEventKind::LobbyFrame(Some(frame)) => {
                write_frame(&mut session, frame).await
            }
            WriterEventKind::MembershipFrame(None) => {
                membership_frames = None;

                WriterFlow::Continue
            }
            WriterEventKind::LobbyFrame(None) => {
                is_lobby_open = false;

                WriterFlow::Continue
            }
        };

        match flow {
            WriterFlow::Continue => {}
            WriterFlow::Close(cause) => {
                close_session(session, cause).await;

                break;
            }
            WriterFlow::End => break,
        }
    }
}

pub async fn close_session(session: Session, cause: CloseCauseKind) {
    let close_reason: CloseReason = CloseReason {
        code: CloseCode::from(cause.close_code()),
        description: cause.description(),
    };

    let _ = tokio::time::timeout(SOCKET_WRITE_TIMEOUT, session.close(Some(close_reason))).await;
}

/// Never finishes without a membership.
async fn receive_membership_frame(membership_frames: &mut Option<mpsc::Receiver<Bytes>>) -> Option<Bytes> {
    match membership_frames {
        Some(membership_frames) => membership_frames.recv().await,
        None => std::future::pending().await,
    }
}

async fn handle_command(
    session: &mut Session,
    membership_frames: &mut Option<mpsc::Receiver<Bytes>>,
    command: WriterCommand,
) -> WriterFlow {
    match command {
        WriterCommand::Frame(frame) => write_frame(session, frame).await,
        WriterCommand::AttachMembership(attached_frames) => {
            *membership_frames = Some(attached_frames);

            WriterFlow::Continue
        }
        WriterCommand::DetachMembership { then_send } => {
            *membership_frames = None;

            match then_send {
                Some(frame) => write_frame(session, frame).await,
                None => WriterFlow::Continue,
            }
        }
        WriterCommand::Ping => {
            let ping_result: Result<Result<(), Closed>, Elapsed> =
                tokio::time::timeout(SOCKET_WRITE_TIMEOUT, session.ping(b"")).await;

            get_write_flow(ping_result)
        }
        WriterCommand::Pong(payload) => {
            let pong_result: Result<Result<(), Closed>, Elapsed> =
                tokio::time::timeout(SOCKET_WRITE_TIMEOUT, session.pong(&payload)).await;

            get_write_flow(pong_result)
        }
        WriterCommand::Close(cause) => WriterFlow::Close(cause),
    }
}

async fn write_frame(session: &mut Session, frame: Bytes) -> WriterFlow {
    let write_result: Result<Result<(), Closed>, Elapsed> =
        tokio::time::timeout(SOCKET_WRITE_TIMEOUT, session.binary(frame)).await;

    get_write_flow(write_result)
}

fn get_write_flow(write_result: Result<Result<(), Closed>, Elapsed>) -> WriterFlow {
    match write_result {
        Ok(Ok(())) => WriterFlow::Continue,
        Ok(Err(Closed)) => WriterFlow::End,
        Err(_) => {
            log::info!("socket write timed out");

            WriterFlow::End
        }
    }
}
```

Create `server/src/socket/connection.rs`:

```rust
use std::net::IpAddr;
use std::time::{Duration, Instant};

use actix_ws::{Message, MessageStream, ProtocolError};
use bytes::Bytes;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::watch::error::RecvError;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::Interval;

use shared::error::AppError;
use shared::game::GameId;
use shared::member::MemberId;
use shared::protocol;
use shared::protocol::{CloseReasonKind, MessageSerialIn, MessageSerialOut};

use crate::game::{GameHandle, GameRequest, GameTaskConfig};
use crate::lobby::lobby_broadcast;
use crate::registries::ServerRegistries;
use crate::shutdown::ServerRunState;
use crate::socket::{
    CloseCauseKind, ConnectionControl, ConnectionId, ConnectionPhase, ProtocolViolationKind, TokenBucket, WriterCommand,
};
use crate::socket::{connection_limit, connection_model, request_dispatch};

pub const PING_PERIOD: Duration = Duration::from_secs(10);
pub const INBOUND_TIMEOUT: Duration = Duration::from_secs(30);

/// The state of one socket, owned by its read task.
pub struct Connection {
    pub connection_id: ConnectionId,
    pub peer_ip: IpAddr,
    pub registries: ServerRegistries,
    pub game_config: GameTaskConfig,
    /// None in the lobby.
    pub joined_game: Option<JoinedGame>,
    pub writer: mpsc::Sender<WriterCommand>,
    pub lobby_sender: mpsc::Sender<Bytes>,
    pub input_bucket: TokenBucket,
    pub request_bucket: TokenBucket,
    pub password_failure_count: u32,
    pub dropped_input_count: u64,
    last_inbound_at: Instant,
}

impl Connection {
    pub fn new(
        connection_id: ConnectionId,
        peer_ip: IpAddr,
        registries: ServerRegistries,
        game_config: GameTaskConfig,
        writer: mpsc::Sender<WriterCommand>,
        lobby_sender: mpsc::Sender<Bytes>,
    ) -> Connection {
        let now: Instant = Instant::now();

        Connection {
            connection_id,
            peer_ip,
            registries,
            game_config,
            joined_game: None,
            writer,
            lobby_sender,
            input_bucket: TokenBucket::create_input_bucket(now),
            request_bucket: TokenBucket::create_request_bucket(now),
            password_failure_count: 0,
            dropped_input_count: 0,
            last_inbound_at: now,
        }
    }

    pub fn enter_game(
        &mut self,
        game_id: GameId,
        member_id: MemberId,
        handle: GameHandle,
        control: watch::Receiver<ConnectionControl>,
    ) {
        self.joined_game = Some(JoinedGame {
            game_id,
            member_id,
            handle,
            control,
        });
    }

    pub fn phase(&self) -> ConnectionPhase {
        match &self.joined_game {
            Some(joined_game) => ConnectionPhase::InGame {
                game_id: joined_game.game_id,
                member_id: joined_game.member_id,
            },
            None => ConnectionPhase::Lobby,
        }
    }

    pub fn return_to_lobby(&mut self) {
        self.joined_game = None;
    }

    /// `Finish` when the writer is gone.
    pub async fn send_to_writer(&self, command: WriterCommand) -> ConnectionFlow {
        let send_result: Result<(), SendError<WriterCommand>> = self.writer.send(command).await;

        match send_result {
            Ok(()) => ConnectionFlow::Continue,
            Err(_) => ConnectionFlow::Finish,
        }
    }

    pub async fn send_message(&self, message_serial_out: &MessageSerialOut) -> ConnectionFlow {
        self.send_to_writer(WriterCommand::Frame(connection_model::encode_frame(message_serial_out))).await
    }
}

pub struct JoinedGame {
    pub game_id: GameId,
    pub member_id: MemberId,
    pub handle: GameHandle,
    pub control: watch::Receiver<ConnectionControl>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionFlow {
    Continue,
    Close(CloseCauseKind),
    /// Ends without a close frame, which could not be written.
    Finish,
}

enum ConnectionEventKind {
    Inbound(Option<Result<Message, ProtocolError>>),
    ControlChanged(Result<ConnectionControl, RecvError>),
    WriterEnded,
    PingDue,
    RunStateChanged(Result<ServerRunState, RecvError>),
}

/// Owns the connection state and never writes to the socket itself.
pub async fn run_connection(
    mut connection: Connection,
    mut stream: MessageStream,
    mut writer_handle: JoinHandle<()>,
    mut run_state: watch::Receiver<ServerRunState>,
) {
    let mut ping_interval: Interval = tokio::time::interval_at(tokio::time::Instant::now() + PING_PERIOD, PING_PERIOD);
    let mut is_run_state_watched: bool = true;

    loop {
        let event: ConnectionEventKind = tokio::select! {
            message = stream.recv() => ConnectionEventKind::Inbound(message),
            control = wait_for_control_change(&mut connection.joined_game) => ConnectionEventKind::ControlChanged(control),
            _ = &mut writer_handle => ConnectionEventKind::WriterEnded,
            _ = ping_interval.tick() => ConnectionEventKind::PingDue,
            run_state = wait_for_run_state_change(&mut run_state), if is_run_state_watched => ConnectionEventKind::RunStateChanged(run_state),
        };

        let flow: ConnectionFlow = match event {
            ConnectionEventKind::Inbound(message) => handle_inbound(&mut connection, message).await,
            ConnectionEventKind::ControlChanged(control) => handle_control_change(&mut connection, control).await,
            ConnectionEventKind::WriterEnded => ConnectionFlow::Finish,
            ConnectionEventKind::PingDue => handle_ping_due(&connection).await,
            ConnectionEventKind::RunStateChanged(Ok(ServerRunState::ShuttingDown)) => {
                ConnectionFlow::Close(CloseCauseKind::Reason(CloseReasonKind::ServerShutdown))
            }
            ConnectionEventKind::RunStateChanged(Ok(ServerRunState::Running)) => ConnectionFlow::Continue,
            ConnectionEventKind::RunStateChanged(Err(_)) => {
                is_run_state_watched = false;

                ConnectionFlow::Continue
            }
        };

        match flow {
            ConnectionFlow::Continue => {}
            ConnectionFlow::Close(cause) => {
                connection.send_to_writer(WriterCommand::Close(cause)).await;

                break;
            }
            ConnectionFlow::Finish => break,
        }
    }

    finish_connection(&connection).await;
}

async fn wait_for_run_state_change(
    run_state: &mut watch::Receiver<ServerRunState>,
) -> Result<ServerRunState, RecvError> {
    run_state.changed().await?;

    Ok(*run_state.borrow_and_update())
}

/// Never finishes outside a game.
async fn wait_for_control_change(joined_game: &mut Option<JoinedGame>) -> Result<ConnectionControl, RecvError> {
    let Some(joined_game) = joined_game else {
        return std::future::pending().await;
    };

    joined_game.control.changed().await?;

    Ok(*joined_game.control.borrow_and_update())
}

async fn handle_inbound(
    connection: &mut Connection,
    message: Option<Result<Message, ProtocolError>>,
) -> ConnectionFlow {
    let message: Message = match message {
        None | Some(Err(ProtocolError::Io(_))) => return ConnectionFlow::Finish,
        Some(Err(ProtocolError::Overflow)) => return close_for_violation(ProtocolViolationKind::Oversize),
        Some(Err(_)) => return close_for_violation(ProtocolViolationKind::Decode),
        Some(Ok(message)) => message,
    };

    connection.last_inbound_at = Instant::now();

    match message {
        Message::Binary(frame) => handle_binary_frame(connection, &frame).await,
        Message::Text(_) => close_for_violation(ProtocolViolationKind::TextFrame),
        Message::Continuation(_) => close_for_violation(ProtocolViolationKind::Decode),
        Message::Ping(payload) => connection.send_to_writer(WriterCommand::Pong(payload)).await,
        Message::Pong(_) | Message::Nop => ConnectionFlow::Continue,
        Message::Close(_) => ConnectionFlow::Close(CloseCauseKind::ClientClosed),
    }
}

async fn handle_binary_frame(connection: &mut Connection, frame: &[u8]) -> ConnectionFlow {
    let decode_result: Result<MessageSerialIn, AppError> = protocol::decode_message_in(frame);

    match decode_result {
        Ok(message_serial_in) => request_dispatch::dispatch(connection, message_serial_in).await,
        Err(_) => close_for_violation(ProtocolViolationKind::Decode),
    }
}

fn close_for_violation(violation: ProtocolViolationKind) -> ConnectionFlow {
    ConnectionFlow::Close(CloseCauseKind::ProtocolViolation(violation))
}

/// A game which ended without closing the connection sends the member back to the lobby.
async fn handle_control_change(
    connection: &mut Connection,
    control: Result<ConnectionControl, RecvError>,
) -> ConnectionFlow {
    match control {
        Ok(ConnectionControl::Close(cause)) => ConnectionFlow::Close(cause),
        Ok(ConnectionControl::Open) => ConnectionFlow::Continue,
        Err(_) => {
            connection.return_to_lobby();
            let game_ended_frame: Bytes = connection_model::encode_frame(&MessageSerialOut::GameEnded);

            connection
                .send_to_writer(WriterCommand::DetachMembership {
                    then_send: Some(game_ended_frame),
                })
                .await
        }
    }
}

async fn handle_ping_due(connection: &Connection) -> ConnectionFlow {
    let is_inbound_timed_out: bool = connection.last_inbound_at.elapsed() >= INBOUND_TIMEOUT;

    if is_inbound_timed_out {
        return ConnectionFlow::Close(CloseCauseKind::Reason(CloseReasonKind::InboundTimeout));
    }

    connection.send_to_writer(WriterCommand::Ping).await
}

async fn finish_connection(connection: &Connection) {
    if let Some(joined_game) = &connection.joined_game {
        let connection_closed: GameRequest = GameRequest::ConnectionClosed {
            connection_id: connection.connection_id,
        };
        let _ = joined_game.handle.requests.send(connection_closed).await;
    }

    lobby_broadcast::unsubscribe(connection.registries, connection.connection_id);
    connection_limit::release_connection(connection.registries, connection.peer_ip);

    log::debug!(
        "connection closed; [connection_id={} dropped_inputs={}]",
        connection.connection_id.0,
        connection.dropped_input_count,
    );
}
```

- [ ] **Step 4: Implement the lobby requests and the endpoint**

Create `server/src/socket/request_dispatch.rs`:

```rust
use std::time::Instant;

use shared::protocol::{
    CloseReasonKind, MessageSerialIn, MessageSerialOut, RejectionKind, RejectionKindSerialOut, RequestKind,
    RequestKindSerialOut,
};

use crate::lobby::lobby_broadcast;
use crate::socket::{CloseCauseKind, Connection, ConnectionFlow, ConnectionPhase, TokenTakeKind};

/// Whether a request may proceed; a shut gate has already answered it.
enum RequestGate {
    Open,
    Shut(ConnectionFlow),
}

/// Only the lobby subscription is answered.
pub async fn dispatch(connection: &mut Connection, message_serial_in: MessageSerialIn) -> ConnectionFlow {
    match message_serial_in {
        MessageSerialIn::SubscribeGameList => subscribe_game_list(connection).await,
        MessageSerialIn::UnsubscribeGameList => {
            lobby_broadcast::unsubscribe(connection.registries, connection.connection_id);

            ConnectionFlow::Continue
        }
        MessageSerialIn::CreateGame { .. }
        | MessageSerialIn::JoinGame { .. }
        | MessageSerialIn::SpectateGame { .. }
        | MessageSerialIn::Respawn { .. }
        | MessageSerialIn::UpdateAppearance { .. }
        | MessageSerialIn::Input(_)
        | MessageSerialIn::RequestSnapshot { .. }
        | MessageSerialIn::LeaveGame => ConnectionFlow::Continue,
    }
}

async fn subscribe_game_list(connection: &mut Connection) -> ConnectionFlow {
    let gate: RequestGate = open_lobby_request(connection, RequestKind::SubscribeGameList).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    lobby_broadcast::subscribe(
        connection.registries,
        connection.connection_id,
        connection.lobby_sender.clone(),
    );

    ConnectionFlow::Continue
}

/// Lobby phase only, within the request rate.
async fn open_lobby_request(connection: &mut Connection, request_kind: RequestKind) -> RequestGate {
    if connection.phase() != ConnectionPhase::Lobby {
        return RequestGate::Shut(reject(connection, request_kind, RejectionKind::AlreadyInGame).await);
    }

    take_request_token(connection, request_kind).await
}

/// Sustained abuse closes the connection.
async fn take_request_token(connection: &mut Connection, request_kind: RequestKind) -> RequestGate {
    let request_token: TokenTakeKind = connection.request_bucket.take(Instant::now());

    match request_token {
        TokenTakeKind::Taken => RequestGate::Open,
        TokenTakeKind::Refused => RequestGate::Shut(reject(connection, request_kind, RejectionKind::RateLimited).await),
        TokenTakeKind::Abused => RequestGate::Shut(ConnectionFlow::Close(CloseCauseKind::Reason(
            CloseReasonKind::RateLimitAbuse,
        ))),
    }
}

async fn reject(connection: &Connection, request_kind: RequestKind, rejection: RejectionKind) -> ConnectionFlow {
    let rejection_message: MessageSerialOut = MessageSerialOut::RequestRejected {
        request: RequestKindSerialOut::from(&request_kind),
        reason: RejectionKindSerialOut::from(&rejection),
    };

    connection.send_message(&rejection_message).await
}
```

Create `server/src/socket/socket_api.rs`:

```rust
use std::net::{IpAddr, Ipv4Addr};

use actix_web::http::header;
use actix_web::{HttpRequest, HttpResponse, rt, web};
use actix_ws::{MessageStream, Session};
use bytes::Bytes;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use shared::protocol;
use shared::protocol::{CloseReasonKind, protocol_limits};

use crate::config;
use crate::game::GameTaskConfig;
use crate::lobby::lobby_broadcast;
use crate::registries::ServerRegistries;
use crate::shutdown::ServerRunState;
use crate::socket::{CloseCauseKind, Connection, ConnectionId, ConnectionLimitReached, WriterCommand};
use crate::socket::{connection, connection_limit, connection_model, socket_writer};

const PROTOCOL_VERSION_PARAMETER: &str = "protocol_version";

/// Shared by every connection of one server.
pub struct SocketContext {
    pub registries: ServerRegistries,
    pub allowed_origins: Vec<String>,
    pub game_config: GameTaskConfig,
    pub run_state: watch::Receiver<ServerRunState>,
}

pub fn configurer(service_config: &mut web::ServiceConfig) {
    service_config.route("/ws", web::get().to(open_socket));
}

/// Version and connection-limit failures upgrade, then close at once, so the client can read the close reason.
async fn open_socket(
    request: HttpRequest,
    body: web::Payload,
    context: web::Data<SocketContext>,
) -> Result<HttpResponse, actix_web::Error> {
    let origin: Option<&str> = request.headers().get(header::ORIGIN).and_then(|origin| origin.to_str().ok());
    let is_origin_allowed: bool =
        origin.is_some_and(|origin| config::is_origin_allowed(&context.allowed_origins, origin));

    if !is_origin_allowed {
        return Ok(HttpResponse::Forbidden().finish());
    }

    let (response, session, stream): (HttpResponse, Session, MessageStream) = actix_ws::handle(&request, body)?;
    let peer_ip: IpAddr =
        request.peer_addr().map_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED), |peer_address| peer_address.ip());
    let protocol_version: Option<u16> = get_protocol_version(request.query_string());

    if protocol_version != Some(protocol::PROTOCOL_VERSION) {
        let cause: CloseCauseKind = CloseCauseKind::Reason(CloseReasonKind::ProtocolVersionMismatch);
        rt::spawn(socket_writer::close_session(session, cause));

        return Ok(response);
    }

    let reservation: Result<(), ConnectionLimitReached> =
        connection_limit::reserve_connection(context.registries, peer_ip);

    if reservation.is_err() {
        rt::spawn(socket_writer::close_session(
            session,
            CloseCauseKind::Reason(CloseReasonKind::ServerFull),
        ));

        return Ok(response);
    }

    start_connection(&context, session, stream, peer_ip);

    Ok(response)
}

fn get_protocol_version(query_string: &str) -> Option<u16> {
    query_string
        .split('&')
        .filter_map(|parameter| parameter.split_once('='))
        .find(|(name, _)| *name == PROTOCOL_VERSION_PARAMETER)
        .and_then(|(_, value)| value.parse::<u16>().ok())
}

/// The read task and the writer task, both on the accepting worker.
fn start_connection(context: &SocketContext, session: Session, stream: MessageStream, peer_ip: IpAddr) {
    let connection_id: ConnectionId = connection_model::allocate_connection_id(context.registries);
    let (writer_sender, writer_receiver): (mpsc::Sender<WriterCommand>, mpsc::Receiver<WriterCommand>) =
        mpsc::channel(socket_writer::WRITER_COMMAND_CHANNEL_CAPACITY);
    let (lobby_sender, lobby_receiver): (mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) =
        mpsc::channel(lobby_broadcast::LOBBY_CHANNEL_CAPACITY);
    let stream: MessageStream = stream.max_frame_size(protocol_limits::INBOUND_FRAME_BYTE_LIMIT);

    let connection: Connection = Connection::new(
        connection_id,
        peer_ip,
        context.registries,
        context.game_config.clone(),
        writer_sender,
        lobby_sender,
    );
    let writer_handle: JoinHandle<()> = rt::spawn(socket_writer::run_socket_writer(
        session,
        writer_receiver,
        lobby_receiver,
    ));
    rt::spawn(connection::run_connection(
        connection,
        stream,
        writer_handle,
        context.run_state.clone(),
    ));

    log::debug!(
        "connection opened; [connection_id={} peer_ip={peer_ip}]",
        connection_id.0
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_protocol_version_reads_the_query_parameter() {
        assert_eq!(get_protocol_version("protocol_version=1"), Some(1));
        assert_eq!(get_protocol_version("other=3&protocol_version=12"), Some(12));
        assert_eq!(get_protocol_version("protocol_version=x"), None);
        assert_eq!(get_protocol_version("version=1"), None);
        assert_eq!(get_protocol_version(""), None);
    }
}
```

Replace the whole contents of `server/src/socket/mod.rs` with:

```rust
pub mod connection;
pub mod connection_limit;
pub mod connection_model;
pub mod rate_limit;
pub mod request_dispatch;
pub mod socket_api;
pub mod socket_writer;

pub use connection::*;
pub use connection_limit::*;
pub use connection_model::*;
pub use rate_limit::*;
pub use request_dispatch::*;
pub use socket_api::*;
pub use socket_writer::*;
```

Replace the whole contents of `server/src/lib.rs` with:

```rust
//! The game server.

pub mod config;
pub mod game;
pub mod lobby;
pub mod password;
pub mod registries;
pub mod shutdown;
pub mod site;
pub mod socket;

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use actix_web::dev::Server;
use actix_web::{App, HttpServer, web};
use tokio::sync::watch;

use shared::error::AppError;

use crate::config::ServerConfig;
use crate::config::tls;
use crate::game::GameTaskConfig;
use crate::game::game_runtime;
use crate::lobby::lobby_broadcast;
use crate::registries::ServerRegistries;
use crate::shutdown::ServerRunState;
use crate::site::site_api;
use crate::socket::SocketContext;
use crate::socket::socket_api;

/// A bound server which runs once awaited.
pub struct CreatedServer {
    pub server: Server,
    pub local_addresses: Vec<SocketAddr>,
    /// `ShuttingDown` closes every connection with `ServerShutdown`.
    pub run_state: watch::Sender<ServerRunState>,
}

pub fn create_server(config: ServerConfig, registries: ServerRegistries) -> Result<CreatedServer, AppError> {
    let (run_state_sender, run_state_receiver): (watch::Sender<ServerRunState>, watch::Receiver<ServerRunState>) =
        watch::channel(ServerRunState::Running);
    let socket_context: web::Data<SocketContext> = web::Data::new(SocketContext {
        registries,
        allowed_origins: config.allowed_origins.clone(),
        game_config: GameTaskConfig::from_server_config(&config),
        run_state: run_state_receiver,
    });
    let site_root: Option<PathBuf> = config.site_root.clone();

    let http_server = HttpServer::new(move || {
        let site_root: Option<PathBuf> = site_root.clone();

        App::new()
            .app_data(socket_context.clone())
            .configure(socket_api::configurer)
            .configure(move |service_config| site_api::configurer(service_config, site_root.as_deref()))
    })
    .disable_signals();

    let bind_address: (IpAddr, u16) = (config.bind_address, config.port);
    let bound_server = match &config.tls_files {
        Some(tls_files) => http_server.bind_rustls_0_23(bind_address, tls::create_rustls_config(tls_files)?)?,
        None => http_server.bind(bind_address)?,
    };
    let local_addresses: Vec<SocketAddr> = bound_server.addrs();

    game_runtime::GAME_RUNTIME.spawn(lobby_broadcast::run_lobby_task(registries));

    Ok(CreatedServer {
        server: bound_server.run(),
        local_addresses,
        run_state: run_state_sender,
    })
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p server --lib --test protocol_errors`

Expected, among the result lines:

```text
test result: ok. 110 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The first line is the library (Task 24 adds the one `socket_api` unit test), the second `protocol_errors`.

- [ ] **Step 6: Commit**

```sh
git status
git add server/src/lib.rs server/src/shutdown/mod.rs server/src/shutdown/shutdown.rs server/src/socket/mod.rs server/src/socket/socket_writer.rs server/src/socket/connection.rs server/src/socket/request_dispatch.rs server/src/socket/socket_api.rs server/tests/helpers/mod.rs server/tests/helpers/test_server.rs server/tests/protocol_errors.rs
git commit -m "websocket endpoint"
```

### Task 25: Game requests over the WebSocket

**Files:**
- Modify: `server/src/socket/request_dispatch.rs`
- Modify: `server/tests/protocol_errors.rs`
- Test: `server/tests/game_lifecycle.rs`, `server/tests/protocol_errors.rs`

The remaining messages of protocol.md (client to server), routed by phase: create (validation, title and slot reservation, password hash, seed from `getrandom`, launch, then the creator's join through the normal path), join and spectate (password check, then `GameRequest::Join` or `Spectate` with `REQUEST_SEND_TIMEOUT`, then attaching the membership queue to the writer only after the reply is `Ok`), respawn, appearance, input (conversion, input bucket, `try_send`), snapshot requests and leave (`DetachMembership { then_send: LeftGame }`). These are the integration tests of testing.md (server integration tests) except the slow consumer, which is Task 26.

- [ ] **Step 1: Write the failing tests**

Create `server/tests/game_lifecycle.rs`:

```rust
mod helpers;

use std::path::PathBuf;
use std::time::Duration;

use helpers::test_server;
use helpers::test_server::{TestClient, TestPlayer, TestServer};
use shared::ability::AbilityPressSet;
use shared::game::{GameModeKind, InputBundle, MemberEvent, Tick};
use shared::geometry::{SubpixelPoint, WorldPoint};
use shared::member::{MemberId, MemberRoleKind};
use shared::protocol::{
    MemberEventSerialOut, MessageSerialIn, MessageSerialOut, PlayerInputSerialIn, RejectionKindSerialOut,
    RequestKindSerialOut, SubpixelPointSerial, TeamChoiceKindSerialIn,
};
use shared::replay;
use shared::replay::{ReplayLog, TickChecksum};

fn has_member_event(bundle: &InputBundle, is_wanted: impl Fn(&MemberEvent) -> bool) -> bool {
    bundle.member_events.iter().any(is_wanted)
}

async fn play_until_member_event(player: &mut TestPlayer, is_wanted: impl Fn(&MemberEvent) -> bool) {
    let playing = async {
        loop {
            let bundle: InputBundle = player.receive_bundle().await;

            if has_member_event(&bundle, &is_wanted) {
                return;
            }
        }
    };

    tokio::time::timeout(test_server::PLAY_TIMEOUT, playing)
        .await
        .expect("the member event arrives within the play timeout");
}

fn create_input(client_tick: Tick, cursor: SubpixelPoint) -> MessageSerialIn {
    MessageSerialIn::Input(PlayerInputSerialIn {
        client_tick: client_tick.0,
        cursor: SubpixelPointSerial::from(&cursor),
        ability_presses: AbilityPressSet::NONE.bits(),
        aim: None,
    })
}

#[actix_web::test]
async fn create_game_joins_the_creator_in_one_message() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut client: TestClient = TestClient::connect(&test_server).await;

    client.send(&test_server::create_game_message("Arena", None, "Blob")).await;

    assert_eq!(
        client.receive().await,
        MessageSerialOut::JoinAccepted {
            game_id: 1,
            member_id: 0
        }
    );
    assert!(matches!(
        client.receive().await,
        MessageSerialOut::Snapshot(snapshot_serial_out) if snapshot_serial_out.state.tick == 0 && snapshot_serial_out.state.members.is_empty()
    ));
    assert!(matches!(
        client.receive().await,
        MessageSerialOut::InputBundle(bundle_serial_out)
            if bundle_serial_out.tick == 1
                && matches!(
                    bundle_serial_out.member_events[..],
                    [MemberEventSerialOut::Joined { member_id: 0, .. }, MemberEventSerialOut::SpawnRequested { member_id: 0, .. }]
                )
    ));
}

#[actix_web::test]
async fn join_game_sends_a_snapshot_then_bundles_and_every_checksum_matches() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    creator.play_until(|player| player.applied_tick().0 >= 3).await;

    let mut joiner: TestPlayer = test_server::join_game(&test_server, creator.game_id, "Joiner").await;
    let first_bundle: InputBundle = joiner.receive_bundle().await;

    assert_eq!(joiner.member_id, MemberId(1));
    assert!(has_member_event(&first_bundle, |member_event| matches!(
        member_event,
        MemberEvent::Joined {
            member_id: MemberId(1),
            ..
        }
    )));

    creator.play_until(|player| player.verified_checksum_count >= 2).await;
    joiner.play_until(|player| player.verified_checksum_count >= 1).await;
}

#[actix_web::test]
async fn spectate_game_adds_a_spectator_which_follows_the_game() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    let mut client: TestClient = TestClient::connect(&test_server).await;

    client
        .send(&MessageSerialIn::SpectateGame {
            game_id: creator.game_id,
            password: None,
            screen_name: String::from("Watcher"),
        })
        .await;
    let mut spectator: TestPlayer = TestPlayer::enter(client).await;
    spectator.play_until(|player| player.state.members.contains_key(&player.member_id)).await;

    assert_eq!(
        spectator.state.members[&spectator.member_id].role,
        MemberRoleKind::Spectator
    );
    spectator.play_until(|player| player.verified_checksum_count >= 1).await;
}

#[actix_web::test]
async fn respawn_brings_back_a_dead_participant() {
    let test_server: TestServer =
        test_server::start_test_server(|config| config.tick_period = Duration::from_millis(10));
    let mut client: TestClient = TestClient::connect(&test_server).await;
    client
        .send(&MessageSerialIn::CreateGame {
            settings: test_server::create_settings_serial_in("Small", GameModeKind::FreeForAll, 300, 8),
            password: None,
            joiner: test_server::create_joiner_serial_in("Mortal", TeamChoiceKindSerialIn::Auto),
        })
        .await;
    let mut player: TestPlayer = TestPlayer::enter(client).await;
    player.play_until(|player| player.get_own_organism().is_some()).await;

    let target: SubpixelPoint = WorldPoint { x: -2000, y: 150 }.to_subpixel_point();

    while player.get_own_organism().is_some() {
        player.client.send(&create_input(player.applied_tick(), target)).await;
        player.receive_bundle().await;
    }

    player
        .client
        .send(&MessageSerialIn::Respawn {
            loadout: test_server::create_loadout_serial(),
            team: TeamChoiceKindSerialIn::Auto,
        })
        .await;
    player.play_until(|player| player.get_own_organism().is_some()).await;
}

#[actix_web::test]
async fn leave_game_answers_left_game_and_the_others_see_the_member_leave() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    let mut leaver: TestPlayer = test_server::join_game(&test_server, creator.game_id, "Leaver").await;

    leaver.client.send(&MessageSerialIn::LeaveGame).await;

    leaver
        .client
        .receive_until(|message_serial_out| *message_serial_out == MessageSerialOut::LeftGame)
        .await;
    play_until_member_event(&mut creator, |member_event| {
        *member_event == MemberEvent::Left { member_id: MemberId(1) }
    })
    .await;
}

#[actix_web::test]
async fn leave_game_then_join_another_game_receives_no_frame_of_the_first_game_after_left_game() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let first_creator: TestPlayer = test_server::create_game(&test_server, "First", "First creator").await;
    let second_creator: TestPlayer = test_server::create_game(&test_server, "Second", "Second creator").await;
    let mut traveller: TestPlayer = test_server::join_game(&test_server, first_creator.game_id, "Traveller").await;
    traveller.receive_bundle().await;

    traveller.client.send(&MessageSerialIn::LeaveGame).await;
    traveller
        .client
        .send(&test_server::join_game_message(
            second_creator.game_id,
            None,
            "Traveller",
        ))
        .await;
    traveller
        .client
        .receive_until(|message_serial_out| *message_serial_out == MessageSerialOut::LeftGame)
        .await;

    let mut second_game_player: TestPlayer = TestPlayer::enter(traveller.client).await;

    assert_eq!(second_game_player.game_id, second_creator.game_id);

    for _ in 0..5 {
        let tick_before: Tick = second_game_player.applied_tick();
        let bundle: InputBundle = second_game_player.receive_bundle().await;

        assert_eq!(bundle.tick, tick_before.next());
    }
}

#[actix_web::test]
async fn finish_connection_removes_the_member_of_a_closed_connection() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    let leaver: TestPlayer = test_server::join_game(&test_server, creator.game_id, "Leaver").await;

    drop(leaver);

    play_until_member_event(&mut creator, |member_event| {
        *member_event == MemberEvent::Left { member_id: MemberId(1) }
    })
    .await;
}

#[actix_web::test]
async fn join_game_admits_only_one_of_two_joins_with_one_name() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    let mut first_client: TestClient = TestClient::connect(&test_server).await;
    let mut second_client: TestClient = TestClient::connect(&test_server).await;

    first_client.send(&test_server::join_game_message(creator.game_id, None, "Twin")).await;
    second_client.send(&test_server::join_game_message(creator.game_id, None, "Twin")).await;
    let replies: [MessageSerialOut; 2] = [first_client.receive().await, second_client.receive().await];

    let rejection: MessageSerialOut = MessageSerialOut::RequestRejected {
        request: RequestKindSerialOut::JoinGame,
        reason: RejectionKindSerialOut::ScreenNameTaken,
    };
    let accepted_count: usize =
        replies.iter().filter(|reply| matches!(reply, MessageSerialOut::JoinAccepted { .. })).count();

    assert_eq!(accepted_count, 1);
    assert!(replies.contains(&rejection));
}

#[actix_web::test]
async fn join_game_admits_only_one_of_two_joins_for_the_last_slot() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator_client: TestClient = TestClient::connect(&test_server).await;
    creator_client
        .send(&MessageSerialIn::CreateGame {
            settings: test_server::create_settings_serial_in("Duel", GameModeKind::FreeForAll, 800, 2),
            password: None,
            joiner: test_server::create_joiner_serial_in("Creator", TeamChoiceKindSerialIn::Auto),
        })
        .await;
    let creator: TestPlayer = TestPlayer::enter(creator_client).await;
    let mut first_client: TestClient = TestClient::connect(&test_server).await;
    let mut second_client: TestClient = TestClient::connect(&test_server).await;

    first_client.send(&test_server::join_game_message(creator.game_id, None, "First")).await;
    second_client.send(&test_server::join_game_message(creator.game_id, None, "Second")).await;
    let replies: [MessageSerialOut; 2] = [first_client.receive().await, second_client.receive().await];

    let rejection: MessageSerialOut = MessageSerialOut::RequestRejected {
        request: RequestKindSerialOut::JoinGame,
        reason: RejectionKindSerialOut::GameFull,
    };
    let accepted_count: usize =
        replies.iter().filter(|reply| matches!(reply, MessageSerialOut::JoinAccepted { .. })).count();

    assert_eq!(accepted_count, 1);
    assert!(replies.contains(&rejection));
}

#[actix_web::test]
async fn run_game_task_removes_an_empty_game_after_the_grace_period() {
    let test_server: TestServer =
        test_server::start_test_server(|config| config.empty_game_grace_period = Duration::from_millis(200));
    let mut creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    creator.client.send(&MessageSerialIn::LeaveGame).await;
    creator
        .client
        .receive_until(|message_serial_out| *message_serial_out == MessageSerialOut::LeftGame)
        .await;

    tokio::time::sleep(Duration::from_millis(600)).await;
    creator.client.send(&test_server::join_game_message(creator.game_id, None, "Late")).await;

    assert_eq!(
        creator.client.receive().await,
        MessageSerialOut::RequestRejected {
            request: RequestKindSerialOut::JoinGame,
            reason: RejectionKindSerialOut::GameNotFound,
        },
    );

    creator.client.send(&test_server::create_game_message("Arena", None, "Creator")).await;

    assert!(matches!(
        creator.client.receive().await,
        MessageSerialOut::JoinAccepted { .. }
    ));
}

#[actix_web::test]
async fn join_game_during_the_grace_period_keeps_the_game() {
    let test_server: TestServer =
        test_server::start_test_server(|config| config.empty_game_grace_period = Duration::from_millis(200));
    let mut creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    creator.client.send(&MessageSerialIn::LeaveGame).await;
    creator
        .client
        .receive_until(|message_serial_out| *message_serial_out == MessageSerialOut::LeftGame)
        .await;

    let mut rejoiner: TestPlayer = test_server::join_game(&test_server, creator.game_id, "Rejoiner").await;
    tokio::time::sleep(Duration::from_millis(600)).await;

    rejoiner.play_until(|player| player.verified_checksum_count >= 1).await;
}

#[actix_web::test]
async fn input_beyond_the_move_budget_is_clamped_and_corrected() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut player: TestPlayer = test_server::create_game(&test_server, "Arena", "Fast").await;
    player.play_until(|player| player.get_own_organism().is_some()).await;
    let spawn_cursor: SubpixelPoint = player.get_own_organism().unwrap().cursor.to_subpixel_point();
    let client_tick: Tick = player.applied_tick();

    let target: SubpixelPoint = SubpixelPoint {
        x: spawn_cursor.x + 500 * 1024,
        y: spawn_cursor.y,
    };
    player.client.send(&create_input(client_tick, target)).await;

    let corrected_cursor: SubpixelPoint = loop {
        let message_serial_out: MessageSerialOut = player.client.receive().await;
        player.apply(&message_serial_out);

        if let MessageSerialOut::CursorCorrected {
            client_tick: corrected_tick,
            cursor,
        } = message_serial_out
        {
            assert_eq!(corrected_tick, client_tick.0);

            break SubpixelPoint::try_from(cursor).unwrap();
        }
    };

    let own_cursor: WorldPoint = player.get_own_organism().unwrap().cursor;

    assert_eq!(corrected_cursor.y, spawn_cursor.y);
    assert!(corrected_cursor.x > spawn_cursor.x);
    assert!(corrected_cursor.x - spawn_cursor.x <= 15 * 3174);
    assert_eq!(own_cursor.x, (corrected_cursor.x + 512).div_euclid(1024));
    assert!(own_cursor.x < target.x / 1024);
}

#[actix_web::test]
async fn replay_capture_records_a_replay_with_the_server_checksums() {
    let replay_directory: PathBuf = std::env::temp_dir().join(format!("bacter-replay-capture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&replay_directory);
    let configured_directory: PathBuf = replay_directory.clone();
    let test_server: TestServer =
        test_server::start_test_server(move |config| config.replay_capture_directory = Some(configured_directory));
    let mut player: TestPlayer = test_server::create_game(&test_server, "Recorded", "Blob").await;

    let mut received_checksums: Vec<(u32, u64)> = Vec::new();

    while received_checksums.len() < 2 {
        let message_serial_out: MessageSerialOut = player.client.receive().await;
        player.apply(&message_serial_out);

        if let MessageSerialOut::StateChecksum { tick, checksum } = message_serial_out {
            received_checksums.push((tick, checksum));
        }
    }

    let replay_path: PathBuf = replay_directory.join(format!("{}.replay", player.game_id));
    let replay_log: ReplayLog = loop {
        let replay_bytes: Vec<u8> = std::fs::read(&replay_path).unwrap_or_default();
        let replay_log: Option<ReplayLog> = replay::read_replay(&replay_bytes).ok();

        if let Some(replay_log) = replay_log.filter(|replay_log| replay_log.input_bundles.len() >= 28) {
            break replay_log;
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    };

    let tick_checksums: Vec<TickChecksum> = replay::run_replay(&replay_log).unwrap();

    for (tick, checksum) in received_checksums {
        let replayed_checksum: TickChecksum = tick_checksums[usize::try_from(tick - 1).unwrap()];

        assert_eq!(
            replayed_checksum,
            TickChecksum {
                tick: Tick(tick),
                checksum
            }
        );
    }
}
```

In `server/tests/protocol_errors.rs`, replace:

```rust
mod helpers;

use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message};

use helpers::test_server;
use helpers::test_server::{TestClient, TestServer};
use server::shutdown::ServerRunState;
use shared::protocol;
use shared::protocol::protocol_limits;
use shared::protocol::{MessageSerialIn, MessageSerialOut, RejectionKindSerialOut, RequestKindSerialOut};
```

with:

```rust
mod helpers;

use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message};

use helpers::test_server;
use helpers::test_server::{TestClient, TestPlayer, TestServer};
use server::shutdown::ServerRunState;
use shared::game::GameModeKind;
use shared::member::MemberId;
use shared::protocol;
use shared::protocol::protocol_limits;
use shared::protocol::{
    MessageSerialIn, MessageSerialOut, RejectionKindSerialOut, RequestKindSerialOut, TeamChoiceKindSerialIn,
    TeamKindSerial,
};
```

Append to the end of `server/tests/protocol_errors.rs`, after one blank line:

```rust
#[actix_web::test]
async fn join_game_checks_the_password() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestClient = TestClient::connect(&test_server).await;
    creator.send(&test_server::create_game_message("Private", Some("secret"), "Owner")).await;
    let creator_player: TestPlayer = TestPlayer::enter(creator).await;
    let mut joiner: TestClient = TestClient::connect(&test_server).await;

    joiner.send(&test_server::join_game_message(creator_player.game_id, None, "Guest")).await;

    assert_eq!(
        joiner.receive().await,
        rejection(RequestKindSerialOut::JoinGame, RejectionKindSerialOut::PasswordRequired),
    );

    joiner
        .send(&test_server::join_game_message(
            creator_player.game_id,
            Some("wrong"),
            "Guest",
        ))
        .await;

    assert_eq!(
        joiner.receive().await,
        rejection(
            RequestKindSerialOut::JoinGame,
            RejectionKindSerialOut::PasswordIncorrect
        ),
    );

    joiner
        .send(&test_server::join_game_message(
            creator_player.game_id,
            Some("secret"),
            "Guest",
        ))
        .await;

    assert_eq!(TestPlayer::enter(joiner).await.member_id, MemberId(1));
}

#[actix_web::test]
async fn join_game_closes_a_connection_after_five_wrong_passwords() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestClient = TestClient::connect(&test_server).await;
    creator.send(&test_server::create_game_message("Private", Some("secret"), "Owner")).await;
    let creator_player: TestPlayer = TestPlayer::enter(creator).await;
    let mut joiner: TestClient = TestClient::connect(&test_server).await;

    for _ in 0..4 {
        joiner
            .send(&test_server::join_game_message(
                creator_player.game_id,
                Some("wrong"),
                "Guest",
            ))
            .await;

        assert_eq!(
            joiner.receive().await,
            rejection(
                RequestKindSerialOut::JoinGame,
                RejectionKindSerialOut::PasswordIncorrect
            ),
        );
    }

    joiner
        .send(&test_server::join_game_message(
            creator_player.game_id,
            Some("wrong"),
            "Guest",
        ))
        .await;

    assert_eq!(
        joiner.receive_close().await,
        (4003, String::from("password failure limit"))
    );
}

#[actix_web::test]
async fn join_game_rejects_a_taken_screen_name() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Blob").await;
    let mut joiner: TestClient = TestClient::connect(&test_server).await;

    joiner.send(&test_server::join_game_message(creator.game_id, None, " Blob ")).await;

    assert_eq!(
        joiner.receive().await,
        rejection(RequestKindSerialOut::JoinGame, RejectionKindSerialOut::ScreenNameTaken),
    );
}

#[actix_web::test]
async fn join_game_rejects_a_full_game() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestClient = TestClient::connect(&test_server).await;
    creator
        .send(&MessageSerialIn::CreateGame {
            settings: test_server::create_settings_serial_in("Duel", GameModeKind::FreeForAll, 800, 2),
            password: None,
            joiner: test_server::create_joiner_serial_in("First", TeamChoiceKindSerialIn::Auto),
        })
        .await;
    let creator_player: TestPlayer = TestPlayer::enter(creator).await;
    let _second_player: TestPlayer = test_server::join_game(&test_server, creator_player.game_id, "Second").await;
    let mut third_client: TestClient = TestClient::connect(&test_server).await;

    third_client.send(&test_server::join_game_message(creator_player.game_id, None, "Third")).await;

    assert_eq!(
        third_client.receive().await,
        rejection(RequestKindSerialOut::JoinGame, RejectionKindSerialOut::GameFull),
    );
}

#[actix_web::test]
async fn join_game_rejects_an_unbalancing_team() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut creator: TestClient = TestClient::connect(&test_server).await;
    creator
        .send(&MessageSerialIn::CreateGame {
            settings: test_server::create_settings_serial_in("Teams", GameModeKind::Skirmish, 800, 8),
            password: None,
            joiner: test_server::create_joiner_serial_in("Red", TeamChoiceKindSerialIn::Team(TeamKindSerial::Red)),
        })
        .await;
    let creator_player: TestPlayer = TestPlayer::enter(creator).await;
    let mut joiner: TestClient = TestClient::connect(&test_server).await;

    joiner
        .send(&MessageSerialIn::JoinGame {
            game_id: creator_player.game_id,
            password: None,
            joiner: test_server::create_joiner_serial_in("Also red", TeamChoiceKindSerialIn::Team(TeamKindSerial::Red)),
        })
        .await;

    assert_eq!(
        joiner.receive().await,
        rejection(
            RequestKindSerialOut::JoinGame,
            RejectionKindSerialOut::TeamUnbalanced {
                requested: TeamKindSerial::Red,
                smaller: TeamKindSerial::Blue,
            },
        ),
    );
}

#[actix_web::test]
async fn dispatch_rejects_requests_outside_their_phase() {
    let test_server: TestServer = test_server::start_test_server(|_| {});
    let mut player: TestPlayer = test_server::create_game(&test_server, "Arena", "Blob").await;
    let mut lobby_client: TestClient = TestClient::connect(&test_server).await;

    lobby_client.send(&MessageSerialIn::LeaveGame).await;
    player.client.send(&test_server::create_game_message("Second", None, "Blob")).await;

    assert_eq!(
        lobby_client.receive().await,
        rejection(RequestKindSerialOut::LeaveGame, RejectionKindSerialOut::NotInGame),
    );
    assert_eq!(
        player
            .client
            .receive_until(|message_serial_out| matches!(message_serial_out, MessageSerialOut::RequestRejected { .. }))
            .await,
        rejection(RequestKindSerialOut::CreateGame, RejectionKindSerialOut::AlreadyInGame),
    );
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p server --no-fail-fast --test game_lifecycle --test protocol_errors`

Expected, after about 10 s:

```text
test result: FAILED. 0 passed; 13 failed; 0 ignored; 0 measured; 0 filtered out
test result: FAILED. 8 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out
```

Every failure panics with `a frame arrives within the receive timeout: Elapsed(())`: the server ignores game requests so far.

- [ ] **Step 3: Implement**

Replace the whole contents of `server/src/socket/request_dispatch.rs` with:

```rust
use std::time::Instant;

use bytes::Bytes;
use tokio::sync::mpsc::error::{SendError, SendTimeoutError, TrySendError};
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;

use shared::ability::Loadout;
use shared::error::AppError;
use shared::game::{GameId, GameSettings, PlayerInput};
use shared::member::{Appearance, Joiner, MemberId, TeamChoiceKind};
use shared::protocol::protocol_limits;
use shared::protocol::{
    AppearanceSerial, CloseReasonKind, GameSettingsSerialIn, JoinerSerialIn, LoadoutSerial, MessageSerialIn,
    MessageSerialOut, PlayerInputSerialIn, RejectionKind, RejectionKindSerialOut, RequestKind, RequestKindSerialOut,
    TeamChoiceKindSerialIn,
};

use crate::game::{ConnectionEndpoint, GameHandle, GameInput, GameRequest, GameReservation};
use crate::game::{game_handle, game_registry, game_supervisor};
use crate::lobby::lobby_broadcast;
use crate::password;
use crate::password::password_failure_limit;
use crate::password::{PasswordAttemptKind, PasswordMatchKind};
use crate::socket::connection_model;
use crate::socket::{
    CloseCauseKind, Connection, ConnectionFlow, ConnectionPhase, Membership, ProtocolViolationKind, TokenTakeKind,
    WriterCommand,
};

/// Whether a request may proceed; a shut gate has already answered it.
enum RequestGate {
    Open,
    Shut(ConnectionFlow),
}

enum JoinKind {
    Participant(Joiner),
    Spectator(String),
}

/// Request payloads which fail their conversion are answered `RequestRejected`; any other invalid message closes
/// the connection.
pub async fn dispatch(connection: &mut Connection, message_serial_in: MessageSerialIn) -> ConnectionFlow {
    match message_serial_in {
        MessageSerialIn::SubscribeGameList => subscribe_game_list(connection).await,
        MessageSerialIn::UnsubscribeGameList => {
            lobby_broadcast::unsubscribe(connection.registries, connection.connection_id);

            ConnectionFlow::Continue
        }
        MessageSerialIn::CreateGame {
            settings,
            password,
            joiner,
        } => create_game(connection, settings, password, joiner).await,
        MessageSerialIn::JoinGame {
            game_id,
            password,
            joiner,
        } => join_game(connection, GameId(game_id), password, joiner).await,
        MessageSerialIn::SpectateGame {
            game_id,
            password,
            screen_name,
        } => spectate_game(connection, GameId(game_id), password, screen_name).await,
        MessageSerialIn::Respawn { loadout, team } => respawn(connection, loadout, team).await,
        MessageSerialIn::UpdateAppearance { appearance } => update_appearance(connection, appearance).await,
        MessageSerialIn::Input(player_input_serial_in) => send_input(connection, player_input_serial_in),
        MessageSerialIn::RequestSnapshot { .. } => request_snapshot(connection).await,
        MessageSerialIn::LeaveGame => leave_game(connection).await,
    }
}

async fn subscribe_game_list(connection: &mut Connection) -> ConnectionFlow {
    let gate: RequestGate = open_lobby_request(connection, RequestKind::SubscribeGameList).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    lobby_broadcast::subscribe(
        connection.registries,
        connection.connection_id,
        connection.lobby_sender.clone(),
    );

    ConnectionFlow::Continue
}

async fn create_game(
    connection: &mut Connection,
    settings_serial_in: GameSettingsSerialIn,
    password: Option<String>,
    joiner_serial_in: JoinerSerialIn,
) -> ConnectionFlow {
    let gate: RequestGate = open_lobby_request(connection, RequestKind::CreateGame).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    let creation: Result<(GameSettings, Joiner, Option<String>), RejectionKind> =
        convert_creation(settings_serial_in, joiner_serial_in, password);
    let (settings, joiner, password): (GameSettings, Joiner, Option<String>) = match creation {
        Ok(creation) => creation,
        Err(rejection) => return reject(connection, RequestKind::CreateGame, rejection).await,
    };

    let reservation_result: Result<GameReservation, RejectionKind> =
        game_registry::reserve_game(connection.registries, &settings.title);
    let reservation: GameReservation = match reservation_result {
        Ok(reservation) => reservation,
        Err(rejection) => return reject(connection, RequestKind::CreateGame, rejection).await,
    };

    let launch: Result<GameHandle, RejectionKind> =
        launch_reserved_game(connection, &reservation, settings, password).await;
    let game_handle: GameHandle = match launch {
        Ok(game_handle) => game_handle,
        Err(rejection) => {
            game_registry::cancel_reservation(connection.registries, &reservation);

            return reject(connection, RequestKind::CreateGame, rejection).await;
        }
    };

    request_membership(
        connection,
        RequestKind::CreateGame,
        reservation.game_id,
        game_handle,
        JoinKind::Participant(joiner),
    )
    .await
}

fn convert_creation(
    settings_serial_in: GameSettingsSerialIn,
    joiner_serial_in: JoinerSerialIn,
    password: Option<String>,
) -> Result<(GameSettings, Joiner, Option<String>), RejectionKind> {
    let settings: GameSettings = GameSettings::try_from(settings_serial_in)?;
    let joiner: Joiner = Joiner::try_from(joiner_serial_in)?;
    let password: Option<String> = protocol_limits::normalize_password(password)?;

    Ok((settings, joiner, password))
}

async fn launch_reserved_game(
    connection: &Connection,
    reservation: &GameReservation,
    settings: GameSettings,
    password: Option<String>,
) -> Result<GameHandle, RejectionKind> {
    let password_hash: Option<String> = match password {
        Some(password) => Some(password::hash_password(password).await?),
        None => None,
    };

    let seed: u64 = getrandom::u64().map_err(|error| {
        log::error!(
            "cannot draw a game seed; [error={}]",
            AppError::from_error("getrandom", Box::new(error))
        );

        RejectionKind::ServerBusy
    })?;

    Ok(game_supervisor::launch_game(
        connection.registries,
        &connection.game_config,
        reservation,
        settings,
        seed,
        password_hash,
    ))
}

async fn join_game(
    connection: &mut Connection,
    game_id: GameId,
    password: Option<String>,
    joiner_serial_in: JoinerSerialIn,
) -> ConnectionFlow {
    let gate: RequestGate = open_lobby_request(connection, RequestKind::JoinGame).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    let joiner: Joiner = match Joiner::try_from(joiner_serial_in) {
        Ok(joiner) => joiner,
        Err(rejection) => return reject(connection, RequestKind::JoinGame, rejection).await,
    };

    enter_existing_game(
        connection,
        RequestKind::JoinGame,
        game_id,
        password,
        JoinKind::Participant(joiner),
    )
    .await
}

async fn spectate_game(
    connection: &mut Connection,
    game_id: GameId,
    password: Option<String>,
    screen_name: String,
) -> ConnectionFlow {
    let gate: RequestGate = open_lobby_request(connection, RequestKind::SpectateGame).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    let screen_name: String = match protocol_limits::normalize_screen_name(&screen_name) {
        Ok(screen_name) => screen_name,
        Err(rejection) => return reject(connection, RequestKind::SpectateGame, rejection).await,
    };

    enter_existing_game(
        connection,
        RequestKind::SpectateGame,
        game_id,
        password,
        JoinKind::Spectator(screen_name),
    )
    .await
}

async fn enter_existing_game(
    connection: &mut Connection,
    request_kind: RequestKind,
    game_id: GameId,
    password: Option<String>,
    join_kind: JoinKind,
) -> ConnectionFlow {
    let password: Option<String> = match protocol_limits::normalize_password(password) {
        Ok(password) => password,
        Err(rejection) => return reject(connection, request_kind, rejection).await,
    };

    let Some(game_handle) = game_registry::get_game(connection.registries, game_id) else {
        return reject(connection, request_kind, RejectionKind::GameNotFound).await;
    };

    let gate: RequestGate = check_game_password(connection, request_kind, &game_handle, password).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    request_membership(connection, request_kind, game_id, game_handle, join_kind).await
}

/// Five failures on one connection close it; an IP past its failure limit is refused without hashing.
async fn check_game_password(
    connection: &mut Connection,
    request_kind: RequestKind,
    game_handle: &GameHandle,
    password: Option<String>,
) -> RequestGate {
    let Some(password_hash) = game_handle.password_hash.clone() else {
        return RequestGate::Open;
    };

    let Some(password) = password else {
        return RequestGate::Shut(reject(connection, request_kind, RejectionKind::PasswordRequired).await);
    };

    let attempt: PasswordAttemptKind = password_failure_limit::check_attempt(
        connection.registries.password_failures_per_ip,
        connection.peer_ip,
        Instant::now(),
    );

    if attempt == PasswordAttemptKind::RateLimited {
        return RequestGate::Shut(reject(connection, request_kind, RejectionKind::RateLimited).await);
    }

    let verification: Result<PasswordMatchKind, RejectionKind> =
        password::verify_password(password, password_hash).await;

    match verification {
        Ok(PasswordMatchKind::Matches) => RequestGate::Open,
        Ok(PasswordMatchKind::DoesNotMatch) => record_password_failure(connection, request_kind).await,
        Err(rejection) => RequestGate::Shut(reject(connection, request_kind, rejection).await),
    }
}

async fn record_password_failure(connection: &mut Connection, request_kind: RequestKind) -> RequestGate {
    password_failure_limit::record_failure(
        connection.registries.password_failures_per_ip,
        connection.peer_ip,
        Instant::now(),
    );
    connection.password_failure_count += 1;

    if connection.password_failure_count >= password::PASSWORD_FAILURE_LIMIT {
        let cause: CloseCauseKind = CloseCauseKind::Reason(CloseReasonKind::PasswordFailureLimit);

        return RequestGate::Shut(ConnectionFlow::Close(cause));
    }

    RequestGate::Shut(reject(connection, request_kind, RejectionKind::PasswordIncorrect).await)
}

async fn request_membership(
    connection: &mut Connection,
    request_kind: RequestKind,
    game_id: GameId,
    game_handle: GameHandle,
    join_kind: JoinKind,
) -> ConnectionFlow {
    let (endpoint, membership): (ConnectionEndpoint, Membership) =
        connection_model::create_membership(connection.connection_id);
    let (reply_sender, reply_receiver): (
        oneshot::Sender<Result<MemberId, RejectionKind>>,
        oneshot::Receiver<Result<MemberId, RejectionKind>>,
    ) = oneshot::channel();
    let request: GameRequest = match join_kind {
        JoinKind::Participant(joiner) => GameRequest::Join {
            endpoint,
            joiner,
            reply: reply_sender,
        },
        JoinKind::Spectator(screen_name) => GameRequest::Spectate {
            endpoint,
            screen_name,
            reply: reply_sender,
        },
    };

    let admission: Result<MemberId, RejectionKind> = send_answered_request(&game_handle, request, reply_receiver).await;
    let member_id: MemberId = match admission {
        Ok(member_id) => member_id,
        Err(rejection) => return reject(connection, request_kind, rejection).await,
    };

    lobby_broadcast::unsubscribe(connection.registries, connection.connection_id);
    connection.enter_game(game_id, member_id, game_handle, membership.control);

    // Attached only after the game accepted; the frames it queued meanwhile wait in the queue.
    connection.send_to_writer(WriterCommand::AttachMembership(membership.outbound)).await
}

/// A full request queue answers `ServerBusy`; a closed one, or a dropped reply, means the game is gone.
async fn send_answered_request<T>(
    game_handle: &GameHandle,
    request: GameRequest,
    reply_receiver: oneshot::Receiver<Result<T, RejectionKind>>,
) -> Result<T, RejectionKind> {
    let send_result: Result<(), SendTimeoutError<GameRequest>> =
        game_handle.requests.send_timeout(request, game_handle::REQUEST_SEND_TIMEOUT).await;

    match send_result {
        Ok(()) => {}
        Err(SendTimeoutError::Timeout(_)) => return Err(RejectionKind::ServerBusy),
        Err(SendTimeoutError::Closed(_)) => return Err(RejectionKind::GameNotFound),
    }

    let reply: Result<Result<T, RejectionKind>, RecvError> = reply_receiver.await;

    reply.unwrap_or(Err(RejectionKind::GameNotFound))
}

async fn respawn(
    connection: &mut Connection,
    loadout_serial: LoadoutSerial,
    team_serial_in: TeamChoiceKindSerialIn,
) -> ConnectionFlow {
    let gate: RequestGate = open_game_request(connection, RequestKind::Respawn).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    let Some(joined_game) = &connection.joined_game else {
        return ConnectionFlow::Continue;
    };

    let (reply_sender, reply_receiver): (
        oneshot::Sender<Result<(), RejectionKind>>,
        oneshot::Receiver<Result<(), RejectionKind>>,
    ) = oneshot::channel();
    let request: GameRequest = GameRequest::Respawn {
        member_id: joined_game.member_id,
        loadout: Loadout::from(loadout_serial),
        team: TeamChoiceKind::from(team_serial_in),
        reply: reply_sender,
    };

    let respawn_result: Result<(), RejectionKind> =
        send_answered_request(&joined_game.handle, request, reply_receiver).await;

    match respawn_result {
        Ok(()) => ConnectionFlow::Continue,
        Err(rejection) => reject(connection, RequestKind::Respawn, rejection).await,
    }
}

async fn update_appearance(connection: &mut Connection, appearance_serial: AppearanceSerial) -> ConnectionFlow {
    let gate: RequestGate = open_game_request(connection, RequestKind::UpdateAppearance).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    let Some(joined_game) = &connection.joined_game else {
        return ConnectionFlow::Continue;
    };

    let request: GameRequest = GameRequest::UpdateAppearance {
        member_id: joined_game.member_id,
        appearance: Appearance::from(appearance_serial),
    };
    let send_result: Result<(), SendTimeoutError<GameRequest>> =
        joined_game.handle.requests.send_timeout(request, game_handle::REQUEST_SEND_TIMEOUT).await;

    match send_result {
        Ok(()) => ConnectionFlow::Continue,
        Err(SendTimeoutError::Timeout(_)) => {
            reject(connection, RequestKind::UpdateAppearance, RejectionKind::ServerBusy).await
        }
        Err(SendTimeoutError::Closed(_)) => {
            reject(connection, RequestKind::UpdateAppearance, RejectionKind::GameNotFound).await
        }
    }
}

/// Inputs outside a game, over the rate limit, or beyond a full input queue are dropped.
fn send_input(connection: &mut Connection, player_input_serial_in: PlayerInputSerialIn) -> ConnectionFlow {
    let Ok(input) = PlayerInput::try_from(player_input_serial_in) else {
        return ConnectionFlow::Close(CloseCauseKind::ProtocolViolation(ProtocolViolationKind::Conversion));
    };

    let Some(joined_game) = &connection.joined_game else {
        return ConnectionFlow::Continue;
    };

    let input_token: TokenTakeKind = connection.input_bucket.take(Instant::now());

    match input_token {
        TokenTakeKind::Taken => {}
        TokenTakeKind::Refused => return ConnectionFlow::Continue,
        TokenTakeKind::Abused => {
            return ConnectionFlow::Close(CloseCauseKind::Reason(CloseReasonKind::RateLimitAbuse));
        }
    }

    let game_input: GameInput = GameInput {
        member_id: joined_game.member_id,
        input,
    };
    let send_result: Result<(), TrySendError<GameInput>> = joined_game.handle.inputs.try_send(game_input);

    if let Err(TrySendError::Full(_)) = send_result {
        connection.dropped_input_count += 1;
    }

    ConnectionFlow::Continue
}

/// Never rejected.
async fn request_snapshot(connection: &mut Connection) -> ConnectionFlow {
    let Some(joined_game) = &connection.joined_game else {
        return ConnectionFlow::Continue;
    };

    let request: GameRequest = GameRequest::RequestSnapshot {
        member_id: joined_game.member_id,
    };
    let _ = joined_game.handle.requests.send(request).await;

    ConnectionFlow::Continue
}

async fn leave_game(connection: &mut Connection) -> ConnectionFlow {
    let gate: RequestGate = open_game_request(connection, RequestKind::LeaveGame).await;

    if let RequestGate::Shut(flow) = gate {
        return flow;
    }

    let Some(joined_game) = &connection.joined_game else {
        return ConnectionFlow::Continue;
    };

    let (reply_sender, reply_receiver): (oneshot::Sender<()>, oneshot::Receiver<()>) = oneshot::channel();
    let request: GameRequest = GameRequest::Leave {
        member_id: joined_game.member_id,
        reply: reply_sender,
    };
    let send_result: Result<(), SendError<GameRequest>> = joined_game.handle.requests.send(request).await;

    if send_result.is_ok() {
        let _ = reply_receiver.await;
    }

    connection.return_to_lobby();
    let left_game_frame: Bytes = connection_model::encode_frame(&MessageSerialOut::LeftGame);

    connection
        .send_to_writer(WriterCommand::DetachMembership {
            then_send: Some(left_game_frame),
        })
        .await
}

/// Lobby phase only, within the request rate.
async fn open_lobby_request(connection: &mut Connection, request_kind: RequestKind) -> RequestGate {
    if connection.phase() != ConnectionPhase::Lobby {
        return RequestGate::Shut(reject(connection, request_kind, RejectionKind::AlreadyInGame).await);
    }

    take_request_token(connection, request_kind).await
}

/// In a game only, within the request rate.
async fn open_game_request(connection: &mut Connection, request_kind: RequestKind) -> RequestGate {
    if connection.joined_game.is_none() {
        return RequestGate::Shut(reject(connection, request_kind, RejectionKind::NotInGame).await);
    }

    take_request_token(connection, request_kind).await
}

/// Sustained abuse closes the connection.
async fn take_request_token(connection: &mut Connection, request_kind: RequestKind) -> RequestGate {
    let request_token: TokenTakeKind = connection.request_bucket.take(Instant::now());

    match request_token {
        TokenTakeKind::Taken => RequestGate::Open,
        TokenTakeKind::Refused => RequestGate::Shut(reject(connection, request_kind, RejectionKind::RateLimited).await),
        TokenTakeKind::Abused => RequestGate::Shut(ConnectionFlow::Close(CloseCauseKind::Reason(
            CloseReasonKind::RateLimitAbuse,
        ))),
    }
}

async fn reject(connection: &Connection, request_kind: RequestKind, rejection: RejectionKind) -> ConnectionFlow {
    let rejection_message: MessageSerialOut = MessageSerialOut::RequestRejected {
        request: RequestKindSerialOut::from(&request_kind),
        reason: RejectionKindSerialOut::from(&rejection),
    };

    connection.send_message(&rejection_message).await
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p server --no-fail-fast --test game_lifecycle --test protocol_errors`

Expected:

```text
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 5: Commit**

```sh
git status
git add server/src/socket/request_dispatch.rs server/tests/game_lifecycle.rs server/tests/protocol_errors.rs
git commit -m "game requests over the websocket"
```

### Task 26: Slow consumer test

**Files:**
- Modify: `server/tests/game_lifecycle.rs`
- Create: `scripts/test/test-slow-consumer.sh`
- Modify: `docs/architecture/dev-workflow.md`
- Test: `server/tests/game_lifecycle.rs`

testing.md (server integration tests): a client which stops reading is removed from the simulation (`MemberEvent::Left`) while others continue. The behaviour comes from Tasks 17 and 24 (write timeout, membership queue closed, `Left`), so this test passes on its first run; it is ignored for its length (decisions above).

- [ ] **Step 1: Write the test**

In `server/tests/game_lifecycle.rs`, replace:

```rust
mod helpers;

use std::path::PathBuf;
use std::time::Duration;

use helpers::test_server;
use helpers::test_server::{TestClient, TestPlayer, TestServer};
use shared::ability::AbilityPressSet;
use shared::game::{GameModeKind, InputBundle, MemberEvent, Tick};
use shared::geometry::{SubpixelPoint, WorldPoint};
use shared::member::{MemberId, MemberRoleKind};
use shared::protocol::{
    MemberEventSerialOut, MessageSerialIn, MessageSerialOut, PlayerInputSerialIn, RejectionKindSerialOut,
    RequestKindSerialOut, SubpixelPointSerial, TeamChoiceKindSerialIn,
};
use shared::replay;
use shared::replay::{ReplayLog, TickChecksum};
```

with:

```rust
mod helpers;

use std::path::PathBuf;
use std::time::Duration;

use futures_util::SinkExt;
use futures_util::stream::{SplitSink, SplitStream};
use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message};

use helpers::test_server;
use helpers::test_server::{TestClient, TestPlayer, TestServer, TestSocket};
use shared::ability::AbilityPressSet;
use shared::game::{GameModeKind, InputBundle, MemberEvent, Tick};
use shared::geometry::{SubpixelPoint, WorldPoint};
use shared::member::{MemberId, MemberRoleKind};
use shared::protocol;
use shared::protocol::{
    MemberEventSerialOut, MessageSerialIn, MessageSerialOut, PlayerInputSerialIn, RejectionKindSerialOut,
    RequestKindSerialOut, SubpixelPointSerial, TeamChoiceKindSerialIn,
};
use shared::replay;
use shared::replay::{ReplayLog, TickChecksum};
```

Append to the end of `server/tests/game_lifecycle.rs`, after one blank line:

```rust
#[actix_web::test]
#[ignore = "fills the loopback socket buffers; run by scripts/test/test-slow-consumer.sh"]
async fn run_socket_writer_drops_a_member_which_stops_reading_while_the_others_continue() {
    let test_server: TestServer =
        test_server::start_test_server(|config| config.tick_period = Duration::from_millis(1));
    let mut creator: TestPlayer = test_server::create_game(&test_server, "Arena", "Creator").await;
    let stalled: TestPlayer = test_server::join_game(&test_server, creator.game_id, "Stalled").await;
    let (mut stalled_sender, _unread_stream): (SplitSink<TestSocket, Message>, SplitStream<TestSocket>) =
        stalled.client.split();
    let snapshot_request: Message =
        Message::Binary(protocol::encode_message_in(&MessageSerialIn::RequestSnapshot { client_tick: 0 }).into());
    actix_web::rt::spawn(async move {
        loop {
            let send_result: Result<(), WebSocketError> = stalled_sender.send(snapshot_request.clone()).await;

            if send_result.is_err() {
                break;
            }

            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });

    play_until_member_event(&mut creator, |member_event| {
        *member_event == MemberEvent::Left { member_id: MemberId(1) }
    })
    .await;
    creator.play_until(|player| player.verified_checksum_count >= 3).await;
}
```

- [ ] **Step 2: Add the script and list it**

Create `scripts/test/test-slow-consumer.sh`:

```sh
#!/usr/bin/env bash
# Runs the ignored game server test of a member which stops reading.

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

cargo test -p server --test game_lifecycle -- --ignored
```

Run: `chmod +x scripts/test/test-slow-consumer.sh`

In `docs/architecture/dev-workflow.md`, replace:

```markdown
- `scripts/test/test-shaders.sh`, `scripts/test/test-growth-statistics.sh`: run the `#[ignore]` tests of [other tests](./testing.md#other-tests) and [growth statistics against the original rules](./testing.md#growth-statistics-against-the-original-rules).
```

with:

```markdown
- `scripts/test/test-shaders.sh`, `scripts/test/test-growth-statistics.sh`: run the `#[ignore]` tests of [other tests](./testing.md#other-tests) and [growth statistics against the original rules](./testing.md#growth-statistics-against-the-original-rules).
- `scripts/test/test-slow-consumer.sh`: runs the `#[ignore]` test of a member which stops reading ([server integration tests](./testing.md#server-integration-tests)).
```

- [ ] **Step 3: Run the test**

Run: `./scripts/test/test-slow-consumer.sh`

Expected, after about 45 s (first run passes, as the task introduction says): `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out`.

Run: `cargo test -p server --test game_lifecycle`

Expected: `test result: ok. 13 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out`.

- [ ] **Step 4: Commit**

```sh
git status
git add server/tests/game_lifecycle.rs scripts/test/test-slow-consumer.sh docs/architecture/dev-workflow.md
git commit -m "slow consumer test"
```

### Task 27: Server binary

**Files:**
- Modify: `server/src/lib.rs`
- Modify: `server/src/main.rs`

`main` loads `.env` (dotenvy, before the logger so `RUST_LOG` applies), starts env_logger, reads `ServerConfig::from_environment` and calls `server::run` with `ServerRegistries::production()` (overview.md, server crate). `run` serves until SIGINT or SIGTERM, then closes every connection with `ServerShutdown` and stops gracefully. The actix server runs as its own task, since `ServerHandle::stop` waits for the server future to process the stop. The closing path is tested by `run_connection_closes_every_connection_when_the_server_shuts_down`; the signal path is checked by hand below.

- [ ] **Step 1: Implement**

Replace the whole contents of `server/src/lib.rs` with:

```rust
//! The game server.

pub mod config;
pub mod game;
pub mod lobby;
pub mod password;
pub mod registries;
pub mod shutdown;
pub mod site;
pub mod socket;

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;

use actix_web::dev::{Server, ServerHandle};
use actix_web::{App, HttpServer, web};
use tokio::sync::watch;
use tokio::task::{JoinError, JoinHandle};

use shared::error::AppError;

use crate::config::ServerConfig;
use crate::config::tls;
use crate::game::GameTaskConfig;
use crate::game::game_runtime;
use crate::lobby::lobby_broadcast;
use crate::registries::ServerRegistries;
use crate::shutdown::ServerRunState;
use crate::site::site_api;
use crate::socket::SocketContext;
use crate::socket::socket_api;

/// Lets the connections write their close frames before the server stops.
const SHUTDOWN_CLOSE_DELAY: Duration = Duration::from_millis(500);

/// A bound server which runs once awaited.
pub struct CreatedServer {
    pub server: Server,
    pub local_addresses: Vec<SocketAddr>,
    /// `ShuttingDown` closes every connection with `ServerShutdown`.
    pub run_state: watch::Sender<ServerRunState>,
}

pub fn create_server(config: ServerConfig, registries: ServerRegistries) -> Result<CreatedServer, AppError> {
    let (run_state_sender, run_state_receiver): (watch::Sender<ServerRunState>, watch::Receiver<ServerRunState>) =
        watch::channel(ServerRunState::Running);
    let socket_context: web::Data<SocketContext> = web::Data::new(SocketContext {
        registries,
        allowed_origins: config.allowed_origins.clone(),
        game_config: GameTaskConfig::from_server_config(&config),
        run_state: run_state_receiver,
    });
    let site_root: Option<PathBuf> = config.site_root.clone();

    let http_server = HttpServer::new(move || {
        let site_root: Option<PathBuf> = site_root.clone();

        App::new()
            .app_data(socket_context.clone())
            .configure(socket_api::configurer)
            .configure(move |service_config| site_api::configurer(service_config, site_root.as_deref()))
    })
    .disable_signals();

    let bind_address: (IpAddr, u16) = (config.bind_address, config.port);
    let bound_server = match &config.tls_files {
        Some(tls_files) => http_server.bind_rustls_0_23(bind_address, tls::create_rustls_config(tls_files)?)?,
        None => http_server.bind(bind_address)?,
    };
    let local_addresses: Vec<SocketAddr> = bound_server.addrs();

    game_runtime::GAME_RUNTIME.spawn(lobby_broadcast::run_lobby_task(registries));

    Ok(CreatedServer {
        server: bound_server.run(),
        local_addresses,
        run_state: run_state_sender,
    })
}

/// Until an interrupt or terminate signal, which closes every connection first.
pub async fn run(config: ServerConfig, registries: ServerRegistries) -> Result<(), AppError> {
    let created_server: CreatedServer = create_server(config, registries)?;
    let server_handle: ServerHandle = created_server.server.handle();
    let mut server_task: JoinHandle<io::Result<()>> = actix_web::rt::spawn(created_server.server);

    log::info!("server listening; [addresses={:?}]", created_server.local_addresses);

    tokio::select! {
        server_result = &mut server_task => return get_server_result(server_result),
        _ = shutdown::wait_for_termination_signal() => {}
    }

    log::info!("server shutting down");
    let _ = created_server.run_state.send(ServerRunState::ShuttingDown);
    tokio::time::sleep(SHUTDOWN_CLOSE_DELAY).await;
    server_handle.stop(true).await;

    get_server_result(server_task.await)
}

fn get_server_result(server_result: Result<io::Result<()>, JoinError>) -> Result<(), AppError> {
    let io_result: io::Result<()> =
        server_result.map_err(|error| AppError::from_error("the server task failed", Box::new(error)))?;

    Ok(io_result?)
}
```

Replace the whole contents of `server/src/main.rs` with:

```rust
use server::config::ServerConfig;
use server::registries::ServerRegistries;
use shared::error::AppError;

#[actix_web::main]
async fn main() -> Result<(), AppError> {
    let _ = dotenvy::dotenv();
    env_logger::Builder::from_default_env()
        .format_timestamp_millis()
        .try_init()
        .map_err(|error| AppError::from_error("cannot start the logger", Box::new(error)))?;

    let config: ServerConfig = ServerConfig::from_environment()?;

    server::run(config, ServerRegistries::production()).await
}
```

- [ ] **Step 2: Verify the binary serves and stops**

Run: `cargo build -p server`

Expected: `Finished` with no `warning`.

Run: `BACTER_PORT=3197 ./scripts/dev/server.sh & sleep 3; curl -s http://127.0.0.1:3197/health; echo; kill -INT $!; sleep 2; jobs`

Expected: the log lines `server listening; [addresses=[127.0.0.1:3197]]` and, after the interrupt, `server shutting down`; `curl` prints `ok`; `jobs` lists nothing, because the server exited. The script loads `./.env` (created by `./setup.sh`), whose `RUST_LOG=info` shows the log lines; dotenvy keeps the `BACTER_PORT` given on the command line, and `cargo run` replaces its own process with the server's, so `$!` is the server.

- [ ] **Step 3: Commit**

```sh
git status
git add server/src/lib.rs server/src/main.rs
git commit -m "server binary"
```

### Task 28: Phase verification

**Files:** none; verification only.

The roadmap's done condition for this phase: the integration tests of `docs/architecture/testing.md` pass against an in-process server, with the rest of the workspace still building and passing for every target.

- [ ] **Step 1: Check formatting**

Run: `cargo fmt --all -- --check`

Expected: no output.

- [ ] **Step 2: Run every test of the workspace**

Run: `cargo test --workspace 2>&1 | grep -E '^test result|FAILED'`

Expected output:

```text
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 110 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 13 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 1.99s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s
test result: ok. 334 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 13.36s
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.81s
test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 6.62s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Every line reads `ok`; in order, the binaries are `protocol_dump`, the `server` library, the `server` binary, `game_lifecycle`, `protocol_errors`, the `shared` library, `determinism`, `forbidden_operations`, `growth_statistics`, `protocol`, `replay`, the `web` targets and the doc tests.

- [ ] **Step 3: Run the ignored slow-consumer test**

Run: `./scripts/test/test-slow-consumer.sh`

Expected: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out`.

- [ ] **Step 4: Check the wasm32 builds**

Run: `./scripts/test/check-wasm.sh`

Expected: exit status 0, with no `warning` or `error` line.

- [ ] **Step 5: Confirm the working tree is clean**

Run: `git status --short`

Expected: no output.

- [ ] **Step 6: List the phase's commits**

Run: `git log --oneline --reverse $(git log --grep '^>>> branch: game-server' --format=%h)..HEAD | cut -d' ' -f2-`

Expected, oldest first (review-fix commits, if any, sit after their task):

```text
phase 5 plan
phase 5 plan review
app error static
game summary from game state
server config
optional tls config
token bucket rate limit
cursor clamp
game runtime
per ip password failure window
password hashing
connection model and game handle
server registries
connection limits
game registry
pending admissions
join spectate and respawn admission
shot slot press predicate
bundle builder
recipients and slow consumers
replay capture
game task
game supervisor
game list
lobby broadcast
static site and health
websocket endpoint
game requests over the websocket
slow consumer test
server binary
```

## Roadmap coverage

- Config, including optional rustls: Tasks 3 and 4; binding in Task 24 (`create_server`).
- Socket read and writer tasks: Task 24 (`connection.rs`, `socket_writer.rs`, `socket_api.rs`), completed by Task 25 (`request_dispatch.rs`).
- Registries: Task 11, with the code over them in Tasks 12, 13 and 22.
- Game task and supervisor: Tasks 19 and 20, on the runtime of Task 7.
- Admission: Tasks 14 and 15.
- Cursor clamp: Task 6, applied in Task 16.
- Bundle builder: Task 16.
- Recipients: Task 17.
- Lobby broadcast: Tasks 21 and 22.
- Passwords: Tasks 8 and 9, checked in Task 25.
- Rate limits: Task 5, applied in Tasks 24 and 25.
- Connection limits: Task 12, applied in Task 24.
- Replay capture: Task 18, started in Task 20, tested end to end in Task 25.
- Static serving and health: Task 23.
- Done condition, testing.md (server integration tests), bullet by bullet:
  - in-process server on `127.0.0.1:0` with `ServerRegistries::leaked()`, tokio-tungstenite clients, `tests/helpers/test_server.rs` shared by both files: Task 24;
  - `game_lifecycle.rs`: create plus join in one message, second client's snapshot then bundles, checksums agreeing with the server's, spectate, die and respawn, leave, leave then join another game, disconnect, two joins with one name, two joins for the last slot, empty-game removal after a 200 ms grace period, rejoin during grace (Task 25); a client that stops reading (Task 26);
  - `protocol_errors.rs`: wrong protocol version, text frame, oversize frame, garbage closing only that connection, rate limits, disallowed `Origin` (Task 24); wrong password, name taken, game full, team unbalanced (Task 25);
  - clamp: `input_beyond_the_move_budget_is_clamped_and_corrected` (Task 25);
  - all checked together in Task 28.
- testing.md (client and server unit tests), server rows: `socket/rate_limit.rs` (Task 5), `game/cursor_clamp.rs` (Task 6), `game/pending_admissions.rs` and `game/membership.rs` (Tasks 14 and 15), `lobby/lobby_broadcast.rs` (Task 22).
- testing.md (protocol tests): the non-increasing `client_tick` rule left over from phase 4 is `queue_input_drops_a_client_tick_not_after_the_last_accepted_one` (Task 16).
- Left to later phases: the client session and its `ClientSession` against this server (phase 6); `game-server.json`, `build-site.sh`, `verify-site-tree.sh`, `build-release.sh` and `run-release-server.sh` (phase 8).

## PR description

**Problem:** bacter-rs had a deterministic simulation and a wire protocol but no server to run games or carry the protocol.

**Solution:** the `server` crate on actix-web and actix-ws: a `/ws` endpoint with origin, protocol-version and connection-limit checks; a read task and a writer task per socket; in-memory registries behind `ServerRegistries`; one tick task per game on a dedicated runtime, with admission, cursor clamping, bundle building, per-member fan-out with slow-consumer handling, checksums, snapshots and optional replay capture; a lobby list broadcast; bcrypt passwords; rate limits; static site serving with `/health`; optional rustls and graceful shutdown. Integration tests drive in-process servers over tokio-tungstenite. Repository: `bacter-rs` only.

## Deviations from the plan

- Teams: `Auto` chooses uniformly among the smallest teams with a server-owned `team_assignment_rng` (the decision above was rewritten during Task 15); Task 19 adds `create_team_assignment_rng` in `game_task.rs`, seeded from `getrandom`, falling back to the game id with a warning.
- Task 14: `TeamSizes::from_members_excluding` in `shared/src/member/team_assignment.rs` (a file the plan does not touch), so a pending respawn counts only on its pending team; `PendingAdmissions` counts saturate, through `pending_admissions::saturating_count` (public, used by `membership.rs`).
- Task 15: `check_new_member` split into `check_screen_name` and `check_member_limit`; `resolve_team` takes `respawning_member: Option<&Member>`; new `choose_auto_team`; new `test_fixture::create_member`, shared by the membership and pending-admission tests.
- Task 5: a take which leaves less than one token does not end the refusing period, so a flood which refills an occasional token is still detected as abuse.
- Task 8: `password_failure_limit::forget_expired_failures`, called by the lobby task on every poll (the Task 8 text was updated); field `failed_at` named `failure_instants`.
- Task 9: `create_bcrypt_error`.
- Task 13: `release_game_slot` and `release_title_and_game_slot` in `game_registry.rs`.
- Task 16: `PendingInput::from_input` replaces `PendingInput::new` (the first input's aim is kept only with a shot-slot press); `MemberInputState::apply_pending_input` and `AppliedPendingInput` hold the per-member step of `build`.
- Task 18: `ReplayCapture::from_record_sender` and `ReplayCapture::finish`; `ReplayRecord` is public.
- Task 19: `close_game` takes the game task and finishes its replay capture first (`GameTask::finish_replay_capture`).
- Task 22: `subscribe` registers the subscriber before sending the current list.
- Task 23: cache headers only on a success or 304 response; file names and the `pkg/` prefix are constants.
- Task 24: `close_after_upgrade` in `socket_api.rs`, `write_within_timeout` in `socket_writer.rs`; a failed SIGTERM handler installation logs a warning. A review fix gave `ConnectionClosed` a send timeout; the phase verification restored `send().await`, as server.md and Task 24 specify.
- Task 25: `GameRequestGate` carries the member id and game handle out of `open_game_request`; `send_request` is split from `send_answered_request`; the unreachable `Continue` fallbacks after the gate became `NotInGame` answers.
- Tests: 11 more `server` unit tests (121, not 110) and 1 more `shared` test (335, not 334); integration tests take their limits from crate constants and bound the death and replay-written loops by `PLAY_TIMEOUT`.
- `GameSummary::from_game_state` counts members without an organism directly; same result.
- Extra commits: `rustfmt password failure limit tests` (Task 11 left the Task 8 tests unformatted) and, in the phase verification, `bind request conversions before matching on them`, `extract the server connection release`, `share the replay file path with the replay capture test` (new `replay_capture::get_replay_path`), and `list the shutdown module in the server module tree` (`docs/architecture/overview.md`).
- Task 28, step 5: the untracked `.idea/` directory keeps `git status --short` from being empty.
