# Overview

- Reference implementation: `/Users/singularity/bacter`, branch `master`.
- "Interpretation" marks the chosen reading where the original is ambiguous.

## Goals
- Full Rust rewrite of Bacter in a new repository `/Users/singularity/bacter-rs`, public as `zacharysiegel/bacter-rs`.
- No licence file and no `license` field in any manifest.
- Released modes only: free for all (`ffa`), skirmish (`skm`), survival (`srv`).
- All abilities: extend/compress, immortality/freeze, neutralize/toxin, spore/secrete, shot carrier.
- Title screen, game browser, create/join/spectate/respawn/pause menus, tutorial, HUD, leaderboard, messages.
- Rules ported faithfully, including the seven deferred questionable rules ([deferred questionable rules](./simulation.md#deferred-questionable-rules)).
- Unified border rule: birth and death both use the death check (cell centre ±6 px).
- Clear bugs fixed, not ported ([bug-fix ledger](./bug-fix-ledger.md)).
- Sync model: server-paced deterministic input broadcast; every client simulates the whole game from per-tick input bundles.
- Simulation bit-identical on wasm32 and native.
- Web first (Leptos menus, wgpu canvas, HTML overlays); `shared` must not preclude a later UniFFI iOS app.
- Runnable entirely on this machine: `cargo leptos watch` plus the game server on its own port.
- Production: prerendered static site plus game WebSocket served by one actix process from one origin.

## Non-goals
- Modes capture the flag, infection, king of the hill, tag; the tag ability.
- Accounts, database, persistence of games or scores (design leaves a seam, [adding an account service later](./server.md#adding-an-account-service-later)).
- Reconnection to a previous member: a reloading player rejoins as a new player.
- Host role after creation; host migration; explicit "end game".
- Client-side rollback prediction of the player's own cells (seam only, [prediction seam](./client-web.md#prediction-seam)).
- iOS app, Android app, WebTransport.
- Font replacement (bundled as-is for now, [third-party material](./third-party.md)).
- Key rebinding UI.
- White or blue world colours (only black worlds were creatable).

## Decisions record
- Binding; the other architecture files elaborate these decisions and do not reopen them.

### Repository
- `/Users/singularity/bacter-rs`, public on GitHub as `zacharysiegel/bacter-rs`.
- No licence file and no `license` field (all rights reserved).
- Default branch `master` everywhere, locally and on GitHub.
- No Spec Kit and no constitution: no `.specify/`, no `specs/`.
- Governance in `docs/conventions/git-workflow.md` and `docs/conventions/testing.md` (final, not provisional).

### Rewrite
- Full Rust rewrite of `/Users/singularity/bacter` (JavaScript: Node, socket.io, p5, React).
- Reference: branch `master` of the original; branch `a3.2.4` holds a broken half-refactor of the server.
- The rewrite replaces the server entirely.
- Models:
  - `/Users/singularity/eafora`: workspace shape, Leptos web, wgpu renderer in `shared`, `docs/conventions/`, `scripts/`.
  - `/Users/singularity/singularity`: actix-web and actix-ws server patterns.
- Multiplatform-capable, primarily web.
- iOS later via UniFFI; nothing iOS is built now, but `shared` must not preclude it.

### Scope
- Released modes only: free for all (`ffa`), skirmish (`skm`, teams), survival (`srv`, rounds and shrinking world).
- All abilities: extend/compress, immortality/freeze, neutralize/toxin, spore/secrete, shoot carrier.
- The tutorial and the title screen.
- No new modes: capture the flag, infection, king of the hill and tag are not implemented.

### Game rules
- Ported faithfully.
- Rules 1 to 7 are questionable but kept as in the original, deferred for comparison once the rewrite is stable ([deferred questionable rules](./simulation.md#deferred-questionable-rules)).
- Rule 8 unifies two checks without changing observable behaviour.

1. A player's own spore or shot acid damages its own cells.
2. Kill credit goes to the last hitter forever.
3. Teammates block each other's growth.
4. An empty site adjacent to k cells is rolled k times per tick (multiplicity kept).
5. Neutralize protects any player's cells inside it.
6. Toxin and neutralize centre on the cursor.
7. Spore and shoot can kill a small organism.
8. Border rule: birth and death boundary checks unified into one rule matching the existing death check (centre ±6 px).

- Clear bugs (crashes, index misalignment, broken validation, wire mismatches, frame-rate-dependent movement and similar) are fixed, not ported ([bug-fix ledger](./bug-fix-ledger.md)).
- Birth phase starts at member index `tick % organism_count`.
- Player cap 2 to 32.

### Accounts and storage
- No accounts and no database for now; added later, without restructuring (design leaves an account-service seam).
- Screen names only; games in server memory.
- Passwords for private games hashed, never stored in plaintext.
- Registries revisited when persistence is added.

### Client
- Rendering: wgpu renderer in `shared`; WebGPU with WebGL2 fallback via `downlevel_webgl2_defaults`; Metal on iOS later.
- Leptos for menus; HUD, leaderboard and messages as HTML overlays over the canvas.
- Font `bacter.ttf` bundled as-is; replaced after the rewrite ([third-party material](./third-party.md)).
- Additions over the original:
  - the message "No room to spawn; press 'R' to try again";
  - the Reconnect button on the title screen.

### Server
- Separate `server` crate on actix-web and actix-ws.
- Dev: `cargo leptos watch` serves the site; the game server runs alongside on its own port.
- Production: site prerendered to static files (as eafora); the actix server serves those files and the game WebSocket from one origin.
- TLS: optional rustls inside actix-web (feature `rustls-0_23`, rustls `0.23.*`), enabled when `BACTER_TLS_CERTIFICATE_PATH` and `BACTER_TLS_PRIVATE_KEY_PATH` are both set; otherwise plain HTTP.
- Cloudflare may proxy in front: Full (strict) uses origin TLS, Flexible does not; the mode is chosen at deploy time.

### Sync model
- Server-paced deterministic input broadcast.
- Simulation in `shared`, bit-identical across wasm32 and native:
  - integer lattice coordinates;
  - seeded RNG, a hand-written `Pcg32`;
  - no HashMap iteration order;
  - logarithm via the pure-Rust `libm` crate or a table;
  - no platform-dependent float operations;
  - 1/1024 px subpixel unit (`SUBPIXELS_PER_PIXEL = 1024`), the only unit finer than a whole pixel.
- The server runs the authoritative simulation at the original 70 ms tick, collects each player's input for each tick, and broadcasts one input bundle per tick.
- Late inputs apply on the next tick the server processes; the server never waits for slow clients and never predicts.
- Every client simulates the whole game from the bundles.
- Input: crosshair position, plus ability presses and shoot aim, tagged with the client's tick, sent once per tick.
- The client moves its crosshair every render frame at the original speed (42.5 px/s).
- The server clamps movement to the speed limit, with a catch-up allowance for delayed batches, using the latest received position per tick.
- The client crosshair is corrected only if the server clamped.
- Periodic state checksums from the server; a client whose checksum differs requests a snapshot.
- Snapshots for join, spectate and resync.
- A slow consumer resyncs via snapshot and is removed only after 10 s stuck.
- Client-side prediction of the player's own cells (rollback) is a possible later layer: room left in the design, not built.

### Wire format
- Transport: binary WebSocket.
- Encoding: bitcode (pinned `0.6.*`, its own `Encode`/`Decode` derive) for every WebSocket message; no serde.
- A protocol version in the handshake.
- A debug tool in `tools/` decodes captured messages.
- A non-real-time HTTP API may use another format ([HTTP endpoints](./protocol.md#http-endpoints)).
- Separate wire types, per eafora `docs/conventions/types.md`:
  - every type that crosses the WebSocket or is written to a replay file has its own dedicated wire type in `shared/src/protocol/`;
  - `XSerialOut` server to client, `XSerialIn` client to server, `XSerial` only when both directions are byte-identical;
  - conversions `From<&Model> for XSerialOut` (or `XSerial`) and `TryFrom<XSerialIn> for Model`, immediately after the wire type;
  - simulation types (`GameState`, `Member`, `Organism`, `CellOccupancy`, `Loadout`, `Appearance`, `Pcg32`, `InputBundle`, `PlayerTickInput`, `MemberEvent`, `Tick`, `MemberId`, `GameId`, ability phases, rounds, world) do not derive bitcode; only the wire types do;
  - reason: the wire types make it obvious which shapes cannot change casually.
- Snapshot payload `GameStateSerialOut`; bundle on the wire `InputBundleSerialOut`; replay files store wire types.
- Checksum: FNV-1a 64 over `bitcode::encode(&GameStateSerialOut::from(&state))`.
- Conversion errors are treated like decode errors.
- Size budgets: `InputBundleSerialOut` at most 128 B for 8 players and 256 B for 16 players.

### Games
- A game persists while anyone is in it.
- The creator has no special role after creation; title, mode, size and password are chosen at creation.
- Creating a game also joins the creator to it in one message.
- An empty game is removed after a 10 s grace period.
- A reloading player rejoins as a new player.

## Workspace layout

```
bacter-rs/
  Cargo.toml                 workspace root
  Cargo.lock
  rust-toolchain.toml        channel = "1.95"
  rustfmt.toml               copied from eafora
  template.env               non-secret server settings
  setup.sh
  CLAUDE.md
  .gitignore                 target/, .env
  shared/                    simulation, protocol, client session, tutorial, title scene, settings trait, renderer (feature)
  server/                    actix-web + actix-ws game server
  web/                       Leptos shell (cargo-leptos)
  tools/protocol_dump/       decodes captured frames and replay logs
  scripts/build/ scripts/dev/ scripts/run/ scripts/test/ scripts/git/
  docs/architecture/ docs/conventions/
```

- No top-level `assets/`. Web static files live in `web/static/`; the font is `web/static/fonts/bacter.ttf`.
- Ability icon pixel paths are data in `shared/src/ability/ability_icon.rs`, not image files.
- No `ios/` crate yet ([multiplatform seams](#multiplatform-seams)).

### Module rules
- Applies to all crates.
- Organised by feature. No `models/`, `handlers/`, `types.rs`.
- `mod.rs` holds only `pub mod x;` declarations, `pub use x::*;` re-exports and doc comments (eafora `shared/src/license/mod.rs`); a cfg-gated `pub mod` and its `pub use` each carry the one-line WHY comment (eafora `shared/src/http/mod.rs`).
- Callers import modules and call functions qualified (`use crate::protocol::state_checksum; state_checksum::get_state_checksum(...)`), which needs the `pub mod`.
- Primary content of feature `x` in `x/x.rs`; siblings `x_model.rs`, `x_db.rs` only when they have content.
- File order: imports, consts/statics, types each followed by its `impl`, free functions.
- `#[cfg]` gating per `docs/conventions/conditional-compilation.md`.
- Every per-target dependency table (`[target.'cfg(...)'.dependencies]`, `.dev-dependencies]`) in every manifest carries a one-line WHY comment framed by the excluded target; this includes the web tables, which eafora leaves uncommented.

### Root `Cargo.toml`
- `resolver = "3"`, members `shared`, `server`, `web`, `tools/protocol_dump`.
- `[workspace.package] edition = "2024"`; each crate `edition.workspace = true`, `publish = false`, `version = "0.0.0"`.
- All versions in `[workspace.dependencies]`, wildcard `"x.y.*"`. Crates present in the eafora or singularity lockfile take that minor; crates in neither (bitcode, actix-files) resolve at the first `cargo update`: bitcode's `0.6.*` pin is the owner's, and the actix-files row reads `resolve` until then.
- Approved by the owner although new to both reference projects: actix-files, getrandom (direct), wasmtime plus the `wasm32-wasip1` target (test tooling, [determinism tests](./testing.md#determinism-tests)).
- Approved by the owner for optional origin TLS ([process](./server.md#process)): rustls and the actix-web feature `rustls-0_23`; `0.23.*` is the rustls minor in both lockfiles.

| Crate                    | Version  | Used by                        |
| ------------------------ | -------- | ------------------------------ |
| bitcode                  | `0.6.*`  | shared `protocol/` only        |
| libm                     | `0.2.*`  | shared                         |
| minimer                  | `2.2.*`  | shared, server, web, tools     |
| log                      | `0.4.*`  | all                            |
| wgpu                     | `30.0.*` | shared (render)                |
| raw-window-handle        | `0.6.*`  | shared (render, non-wasm32)    |
| bytemuck                 | `1.25.*` | shared (render)                |
| web-sys                  | `0.3.*`  | shared (render, wasm32), web   |
| actix-web                | `4.13.*` | server (feature `rustls-0_23`) |
| rustls                   | `0.23.*` | server                         |
| actix-ws                 | `0.4.*`  | server                         |
| actix-files              | resolve  | server                         |
| tokio                    | `1.52.*` | server, web (ssr)              |
| futures-util             | `0.3.*`  | server                         |
| bytes                    | `1.11.*` | server                         |
| dashmap                  | `6.1.*`  | server                         |
| bcrypt                   | `0.17.*` | server                         |
| getrandom                | `0.4.*`  | server                         |
| env_logger               | `0.11.*` | server, tools                  |
| dotenvy                  | `0.15.*` | server                         |
| clap                     | `4.6.*`  | tools                          |
| leptos                   | `0.8.*`  | web                            |
| leptos_meta              | `0.8.*`  | web                            |
| leptos_axum              | `0.8.*`  | web (ssr)                      |
| axum                     | `0.8.*`  | web (ssr)                      |
| any_spawner              | `0.3.*`  | web (ssr)                      |
| wasm-bindgen             | `0.2.*`  | web (hydrate)                  |
| wasm-bindgen-futures     | `0.4.*`  | web (hydrate)                  |
| js-sys                   | `0.3.*`  | web (hydrate)                  |
| console_log              | `0.2.*`  | web (hydrate)                  |
| console_error_panic_hook | `0.1.*`  | web (hydrate)                  |
| wasm-bindgen-test        | `0.3.*`  | web dev (wasm32)               |
| tokio-tungstenite        | `0.26.*` | server dev (integration tests) |

- Not taken from eafora: leptos_router (single route), leptos_i18n, gloo-net, sqlx, rusqlite, reqwest, secr, uniffi, uuid, serde, serde_json.
- No `rand`, `rand_core`, `rand_pcg`: the simulation RNG is hand-written (owner decision, [determinism rules](./determinism.md#determinism-rules)).
- No `serde` until a consumer exists (a later HTTP API); bitcode uses its own derive and the browser parses `game-server.json` with `JSON.parse`.
- bitcode (owner decision) is derived only by the wire types in `shared/src/protocol/` ([wire types](./protocol.md#wire-types)); no other module or crate names it.
- PEM parsing for TLS uses `rustls::pki_types::pem::PemObject` (enabled by rustls's default `std` feature); no `rustls-pemfile` (superseded by `PemObject`) and no direct `rustls-pki-types`.
- `cargo update -p wasm-bindgen --precise 0.2.122` after the first resolve, to match the installed `wasm-bindgen` CLI.
- `[profile.wasm-release]`: `inherits = "release"`, `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = true` (copied from eafora).
- `rust-toolchain.toml`: `channel = "1.95"` only; `setup.sh` adds the wasm32 target.

### `shared` crate
- Responsibility: everything both ends must agree on, plus platform-independent client logic.
  - Simulation (pure, deterministic).
  - Protocol types and codec.
  - Client session (sans-IO), crosshair controller, tutorial, title scene.
  - Settings trait.
  - Renderer behind feature `render`.
- `[lib]` default; no binding code (no wasm-bindgen exports, no UniFFI).
- Dependencies: bitcode (`protocol/` only), libm, minimer, log.
- Feature `render = ["dep:wgpu", "dep:raw-window-handle", "dep:bytemuck"]`; off by default so `server` never links wgpu.
- Per-target tables (each with a one-line WHY comment):
  - `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`: wgpu `metal`, `vulkan`, `wgsl` (optional); raw-window-handle (optional).
  - `[target.'cfg(target_arch = "wasm32")'.dependencies]`: wgpu `webgpu`, `webgl`, `wgsl` (optional); web-sys `HtmlCanvasElement` (optional, enabled by `render`).
- Module tree (`shared/src/`):

```
lib.rs
error/            error.rs               AppError (minimer `define_app_error!`) and AppErrorStatic (`define_app_error_static!`, `Send`) with both `From` conversions, as eafora
geometry/         geometry.rs            WorldPoint, SubpixelPoint, SubpixelVector, LatticeCoordinate, conversions
random/           pcg32.rs               Pcg32, from_seed, from_parts
world/            world.rs               World, WorldShapeKind, WorldBounds, contains_cell, shrink
organism/         organism.rs            Organism, centroid, exposed/adjacent enumeration
                  cell_occupancy.rs      CellOccupancy (dense bitmap, canonical order), from_tight_bitmap
                  growth.rs              birth and natural death phases
                  growth_chance_table.rs static chance tables
                  spawn.rs               spawn position search, place_organism (explicit position, no RNG)
ability/          ability_model.rs       Loadout, ability kinds, phases, AbilityPressSet, AimVector
                  ability_constants.rs   durations, cooldowns, radii, speeds
                  activation.rs          key presses to state changes
                  projectile.rs          spore and shot launch, flight, target selection
                  damage.rs              acid, toxin, neutralize protection
                  ability_icon.rs        pixel pen paths
member/           member.rs              Member, MemberId, MemberRoleKind, Appearance, OrganismColorKind, SkinKind, TeamKind
                  joiner.rs              Joiner, TeamChoiceKind (converted from JoinerSerialIn)
                  scoreboard.rs          Score, LeaderboardRow, get_leaderboard_rows
                  team_assignment.rs     balance rule, smallest-team candidates
round/            round.rs               RoundState, RoundPhase, transitions, countdown
game/             game_state.rs          GameState
                  game_settings.rs       GameSettings, GameModeKind, validation
                  game_summary.rs        GameSummary
                  step.rs                step(), tick order
                  input_bundle.rs        InputBundle, PlayerTickInput, MemberEvent (as consumed by step)
                  player_input.rs        PlayerInput (converted from PlayerInputSerialIn)
                  simulation_event.rs    SimulationEvent, SpawnRejectionKind
protocol/         protocol.rs            PROTOCOL_VERSION; encode/decode of messages, replay records and game state (the only bitcode calls)
                  message_in.rs          MessageSerialIn, GameSettingsSerialIn, JoinerSerialIn, TeamChoiceKindSerialIn, PlayerInputSerialIn
                  message_out.rs         MessageSerialOut, GameSnapshotSerialOut
                  game_state_serial.rs   GameStateSerialOut, GameSettingsSerialOut, GameModeKindSerial, WorldShapeKindSerial, world, round and Pcg32 wire types
                  member_serial.rs       MemberSerialOut, ScoreSerialOut, LoadoutSerial, FirstAbilityKindSerial, SecondAbilityKindSerial, ThirdAbilityKindSerial, AppearanceSerial, OrganismColorKindSerial, SkinKindSerial, MemberRoleKindSerialOut, TeamKindSerial
                  organism_serial.rs     OrganismSerialOut, CellOccupancySerialOut, ability phase and projectile wire types
                  geometry_serial.rs     WorldPointSerialOut, SubpixelPointSerial, SubpixelVectorSerialOut, LatticeCoordinateSerialOut, AimVectorSerial
                  input_bundle_serial.rs InputBundleSerialOut, PlayerTickInputSerialOut, MemberEventSerialOut
                  replay_serial.rs       ReplayHeaderSerialOut
                  lobby.rs               GameSummarySerialOut
                  rejection.rs           RequestKind, RejectionKind, SettingFieldKind, RangeBoundKind and their SerialOut wire types
                  close_reason.rs        CloseReasonKind, close codes, client notices
                  state_checksum.rs      get_state_checksum
                  protocol_limits.rs     length and count limits
                  (every wire type is followed by its conversions, protocol.md#wire-types)
replay/           replay.rs              ReplayHeader, ReplayLog, ReplayReadError, read and write of the record stream, run_replay
play/             client_session.rs      ClientSession, ClientEffect
                  frame_pacing.rs        FramePacer, FrameOutput (dt clamp, local-bundle accumulator, interpolation alpha)
                  active_scene.rs        ActiveScene (Online, Tutorial, Title), advance_frame dispatch
                  own_member_state.rs    OwnMemberStateKind (client-web.md#client-session-frame-pacing-and-driving)
                  player_controls.rs     PlayerControls (KeyboardState plus CrosshairController, owned by the platform)
                  crosshair.rs           CrosshairController, SentCursorHistory
                  game_key.rs            GameKeyKind, KeyboardState, press edge accumulation
                  interpolation.rs       projectile interpolation
                  message_text.rs        get_message_text
tutorial/         tutorial.rs            Tutorial, TutorialTaskKind
                  tutorial_bot.rs
title/            title_scene.rs         TitleScene (10 dummy organisms)
settings/         settings.rs            SettingKey, SettingValue, SettingsStore, Setting (eafora pattern)
render/           (feature render)       client-web.md#renderer
```

### `server` crate
- Responsibility: WebSocket endpoint, game registry, one task per game, lobby list broadcast, password hashing, production static file serving.
- `[lib]` plus `[[bin]] name = "server"`; the lib exists so integration tests start the server in-process.
- Dependencies: shared (no `render`), actix-web (feature `rustls-0_23`), rustls, actix-ws, actix-files, tokio (`full`, Singularity default; also supplies `rt-multi-thread` for the game runtime, [game task](./server.md#game-task)), futures-util, bytes, dashmap, bcrypt, getrandom, log, env_logger, dotenvy, minimer.
- Dev dependencies: tokio-tungstenite.
- `shared` and `web` keep minimal tokio feature sets (`web` ssr only).
- Module tree (`server/src/`):

```
main.rs                      env_logger init, ServerConfig::from_environment, server::run with ServerRegistries::production()
lib.rs                       pub mod declarations; run(), create_server(config, registries)
registries/  registries.rs   ServerRegistries (Copy struct of `&'static` references to every registry static; tests leak their own)
config/      config.rs       ServerConfig
             tls.rs          optional rustls ServerConfig from the PEM certificate chain and private key (server.md#process)
socket/      socket_api.rs   configurer(): GET /ws, Origin check, actix_ws::handle
             connection.rs   per-connection read/control task
             socket_writer.rs     per-connection writer task, write timeout
             connection_model.rs  ConnectionId, ConnectionPhase, ConnectionControl, Membership
             connection_limit.rs  server-wide and per-IP connection counters
             request_dispatch.rs  payload conversions (`TryFrom<...SerialIn>`) and routing by phase
             rate_limit.rs   TokenBucket
game/        game_registry.rs     statics GAMES, GAME_TITLES, NEXT_GAME_ID, GAME_COUNT; reserve/insert/lookup/remove over `ServerRegistries`
             game_runtime.rs      static GAME_RUNTIME (multi-threaded tokio runtime)
             game_handle.rs       GameHandle, GameRequest, GameInput
             game_task.rs         tick loop
             game_supervisor.rs   awaits the task, removes the registry entry, logs panics
             membership.rs        join/spectate/respawn validation inside the task
             pending_admissions.rs     PendingAdmissions ledger (names, spawns, team sizes, next member id)
             cursor_clamp.rs      speed clamp with catch-up allowance
             bundle_builder.rs    pending commands to InputBundle
             recipients.rs        per-connection fan-out, slow-consumer policy
             replay_capture.rs    optional replay record stream, writer task
lobby/       lobby_broadcast.rs   static LOBBY_SUBSCRIBERS: LazyLock<DashMap<ConnectionId, mpsc::Sender<Bytes>>>, lobby task
             game_list.rs         `GameList` construction from each `GameHandle.summary`
password/    password.rs          hash/verify on web::block, static PASSWORD_CHECK_SEMAPHORE
             password_failure_limit.rs per-IP failure window
site/        site_api.rs          configurer(): actix-files, cache headers, GET /health
```

- Imperative routing: every `*_api.rs` exposes `pub fn configurer(config: &mut web::ServiceConfig)`; no route macros.

### `web` crate
- Responsibility: Leptos UI tree, browser glue (WebSocket, localStorage, requestAnimationFrame, canvas, keyboard).
- `crate-type = ["cdylib", "rlib"]`; `#![recursion_limit = "512"]` in `lib.rs` and `main.rs`.
- Dependencies: shared with `render`, leptos, leptos_meta, log; optional per feature.
- Feature `hydrate`: wasm-bindgen, wasm-bindgen-futures, js-sys, web-sys, console_log, console_error_panic_hook, `leptos/hydrate`.
- Feature `ssr`: leptos_axum, axum, any_spawner, tokio (`rt`, `rt-multi-thread`, `net`, `macros`), `leptos/ssr`, `leptos_meta/ssr`.
- web-sys features: `Window`, `Document`, `Location`, `Storage`, `Performance`, `HtmlCanvasElement`, `HtmlElement`, `Element`, `DomRect`, `EventTarget`, `Event`, `KeyboardEvent`, `MouseEvent`, `FocusEvent`, `WebSocket`, `BinaryType`, `MessageEvent`, `CloseEvent`, `ErrorEvent`, `Response`, `RequestInit`.
- `[target.'cfg(target_arch = "wasm32")'.dependencies]`: `log` with `release_max_level_info`; WHY comment ([module rules](#module-rules)).
- `[target.'cfg(target_arch = "wasm32")'.dev-dependencies]`: wasm-bindgen-test (localStorage store tests only); WHY comment ([module rules](#module-rules)).
- `[package.metadata.leptos]` as eafora except the ports: `output-name = "bacter"`, `site-root = "target/site"`, `site-pkg-dir = "pkg"`, `style-file = "style/main.scss"`, `assets-dir = "static"`, `site-addr = "127.0.0.1:3011"`, `reload-port = 3012`, `bin-features = ["ssr"]`, `lib-features = ["hydrate"]`, both default-features off, `lib-profile-release = "wasm-release"`.
  - Own ports so the eafora and bacter-rs dev servers can run together, and so `build-site.sh` port detection does not trip on eafora.
- Hydrate gating (web-sys, js-sys and wasm-bindgen are `hydrate`-only, so the `ssr` build must compile none of their users):
  - Whole browser-only modules are gated at their `pub mod` with a one-line WHY comment: `client/` (`// browser runtime glue; the ssr build compiles none of it`), `canvas/driver.rs`, `canvas/keyboard.rs`, `message/text_measurement.rs`.
  - A component which needs one browser call holds a same-file `#[cfg(feature = "hydrate")] mod browser` / `#[cfg(feature = "ssr")] mod server` pair exposing the same function (the ssr one a no-op), per `conditional-compilation.md`.
  - Component files otherwise never name web-sys, js-sys or wasm-bindgen.
- Module tree (`web/src/`), "(hydrate)" and "(ssr)" mark gated modules:

```
lib.rs  main.rs  app.rs       shell() (ssr), App (screen switch)
server.rs                     (ssr) dev serve(), prerender
client/                       (hydrate)
         hydrate.rs           single #[wasm_bindgen] entry
         game_socket.rs       web-sys WebSocket wrapper
         game_server_config.rs  reads static/game-server.json
         local_storage_settings.rs  SettingsStore over localStorage
         fullscreen.rs
screen/  screen.rs            Screen enum, navigation, Escape mapping
canvas/  game_canvas.rs       canvas component (mount through a browser/server submodule pair)
         driver.rs            (hydrate) thread_local DRIVER, rAF scheduling, renderer ownership
         keyboard.rs          (hydrate) KeyboardEvent to GameKeyKind
title/   title_menu.rs        includes the connection notice line (client-web.md#shell)
browser/ game_browser.rs
menu/    menu_form.rs         generic form engine, field kinds, per-mode visibility
         create_menu.rs  join_menu.rs  spectate_menu.rs  respawn_menu.rs  pause_menu.rs
hud/     ability_bar.rs  ability_icon_view.rs  name_labels.rs
leaderboard/ leaderboard.rs
message/ message_box.rs
         text_measurement.rs  (hydrate) DOM text width
tutorial/ tutorial_view.rs
style/   main.scss + partials (_tokens, _reset, _typography, _layout, components/_*.scss)
static/  fonts/bacter.ttf, favicon, robots.txt, game-server.json
tests/   prerender.rs         #![cfg(feature = "ssr")]
```

### `tools/protocol_dump`
- Binary; dependencies shared, clap, minimer, env_logger.
- `protocol_dump frame --direction client-to-server|server-to-client [--hex|--base64] [FILE]`: decodes one frame (stdin or file) and prints the `Debug` form.
- `protocol_dump replay FILE [--checksums]`: decodes a `ReplayLog`, prints bundles, optionally the per-tick checksums from re-running it.
- Frames come from browser devtools (copied as base64) or from server replay capture.

### `scripts/` and `docs/`
- `scripts/`: [scripts](./dev-workflow.md#scripts).
- `docs/conventions/`: [planned contents](./dev-workflow.md#docsconventions).
- `docs/architecture/`: this architecture, one file per topic, indexed by [README.md](./README.md); `third-party.md` covers the font and its FontStruct non-commercial licence.

## Multiplatform seams
- `shared` contains no browser or OS calls outside `render/surface.rs` submodules.
- Platform services:
  - `SettingsStore` trait (implemented per platform).
  - Transport: `ClientSession` is sans-IO (`receive`, outgoing frames returned as `ClientEffect::SendFrame`, [client session, frame pacing and driving](./client-web.md#client-session-frame-pacing-and-driving)); each platform owns its socket.
  - Frame logic lives in `shared` (`ActiveScene::advance_frame`, `FramePacer`); a platform only schedules frames, forwards events and renders.
  - Time and randomness are parameters (`now_milliseconds`, seeds), not traits.
  - Input: platforms map native events to `GameKeyKind` and pointer offsets to `AimVector`.
  - Surface: `WgpuSurface::from_canvas` (wasm32) and `from_window_handle` (other targets).
- Later `ios/` crate: `staticlib`, UniFFI proc-macro exports (`create_renderer`, `session_receive`, `advance_frame`, `key_changed`), a `SettingsStore` implementation over a Swift callback interface, main-thread checks as in eafora, `FfiError` at the boundary; Swift owns `URLSessionWebSocketTask`.
- Gating convention: `docs/conventions/conditional-compilation.md` (gate a submodule plus its `pub use`, WHY comment framed by the excluded target).

## Deferred work
- Compare the seven questionable rules ([deferred questionable rules](./simulation.md#deferred-questionable-rules)) against alternatives once the rewrite is stable.
- Growth-neighbour multiplicity: compare rule 4 against deduplicated sites with an explicit neighbour-weighted formula, using the statistics test ([growth statistics against the original rules](./testing.md#growth-statistics-against-the-original-rules)).
- Client-side rollback prediction of the player's own cells ([prediction seam](./client-web.md#prediction-seam)).
- Replace `bacter.ttf` (FontStruct non-commercial licence, [third-party material](./third-party.md)).
- Accounts and persistence ([adding an account service later](./server.md#adding-an-account-service-later)); the in-memory registries ([registries](./server.md#registries)) are replaced or backed by the persistent store at that point.
- iOS app via UniFFI ([multiplatform seams](#multiplatform-seams)).
- WebTransport as an alternative transport.
