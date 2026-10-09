# Rust rewrite roadmap

> **For agentic workers:** each phase below gets its own detailed plan (`docs/plans/<date>-phase-<n>-<slug>.md`), written against the code as it stands when the phase starts. REQUIRED SUB-SKILL for executing a phase plan: superpowers:subagent-driven-development or superpowers:executing-plans. Steps in phase plans use checkbox (`- [ ]`) syntax.

**Goal:** a faithful, bug-fixed Rust rewrite of Bacter, web first, as specified in `docs/architecture/`.

**Architecture:** deterministic simulation in `shared`, server-paced input broadcast over a binary WebSocket (bitcode wire types), actix-web game server, Leptos shell with a wgpu renderer.

**Tech stack:** Rust 1.95, actix-web, actix-ws, tokio, bitcode, libm, Leptos 0.8, cargo-leptos, wgpu 30.

---

## Branches

- Linear stack: each phase branches from the previous phase's branch; each PR targets its parent branch.
- Every branch starts with the empty marker commit `>>> branch: <name>`.
- Phase 0 is `architecture-design` (this document and `docs/architecture/`).

| Phase | Branch                  | Parent                  |
| ----- | ----------------------- | ----------------------- |
| 1     | `workspace-scaffold`    | `architecture-design`   |
| 2     | `simulation-growth`     | `workspace-scaffold`    |
| 3     | `simulation-abilities`  | `simulation-growth`     |
| 4     | `protocol`              | `simulation-abilities`  |
| 5     | `game-server`           | `protocol`              |
| 6     | `client-session`        | `game-server`           |
| 7     | `renderer`              | `client-session`        |
| 8     | `web-client`            | `renderer`              |
| 9     | `verification`          | `web-client`            |

## Phases

### Phase 1: workspace scaffold
- Root `Cargo.toml` (members, `[workspace.dependencies]`, `wasm-release` profile), `rust-toolchain.toml`, `rustfmt.toml`, `template.env`, `setup.sh`, `CLAUDE.md`.
- Crates `shared`, `server`, `web`, `tools/protocol_dump` compiling as empty shells with their final feature flags and per-target tables.
- `docs/conventions/` (README, logging, conditional-compilation, shading, types, git-workflow, testing).
- `scripts/` skeleton: dev, build, run, test, git scripts that already have something to run.
- Done when: `cargo build --workspace`, `cargo test --workspace`, `./scripts/test/check-wasm.sh` pass.

### Phase 2: simulation core and growth
- `geometry`, `random/pcg32`, `world`, `organism` (`CellOccupancy`, exposed and adjacent enumeration, growth chance tables, birth and natural death, spawn search, `place_organism`), `member` data, `game_state`, `game_settings`.
- `step` with the tick order phases that exist so far (membership, cursors, births, deaths, death bookkeeping, re-tighten).
- Forbidden-operations source scan test; golden chance-table digest; `Pcg32` reference vectors.
- Done when: unit tests for every growth, border and collision rule pass; the growth statistics test matches the reference means.

### Phase 3: abilities, rounds, scoring
- `ability` (activation, projectiles, damage, constants, icons), `round`, `member/scoreboard`, `member/team_assignment`, `simulation_event`, `checksum`; the complete tick order.
- Determinism tests: 10,000-tick scripted game twice, golden checksum sequence.
- Done when: every ability, round and scoring unit test passes and the determinism tests are green.

### Phase 4: protocol
- Wire types with `From`/`TryFrom` conversions, `MessageSerialIn`/`MessageSerialOut`, encode and decode, `validate()`, limits, rejection and close-reason kinds, replay format (`replay/`), snapshot.
- `tools/protocol_dump`.
- Done when: round-trip, model-to-wire-to-model, malformed-input and size-budget tests pass; replay fixture tests pass.

### Phase 5: game server
- `server`: config (including optional rustls), socket read and writer tasks, registries, game task and supervisor, admission, cursor clamp, bundle builder, recipients, lobby broadcast, passwords, rate limits, connection limits, replay capture, static serving, health.
- Done when: the integration tests of `docs/architecture/testing.md` pass against an in-process server.

### Phase 6: client session
- `shared/src/play/` (client session, frame pacing, crosshair, keys, own member state, message text, interpolation), `tutorial/`, `title/`, `settings/`.
- Done when: client and tutorial unit tests pass, and a headless test drives two `ClientSession`s against the in-process server to identical checksums.

### Phase 7: renderer
- `shared/src/render/` behind the `render` feature: surface, cell and shape pipelines, WGSL, scene construction, camera.
- Done when: `RenderScene` unit tests and the ignored pipeline-compile test pass.

### Phase 8: web client
- `web`: shell, screens, menu form engine, game browser, HUD, leaderboard, message box, name labels, tutorial view, driver, keyboard, socket, local storage settings, styles, font, `game-server.json`, prerender; `build-site.sh`, `verify-site-tree.sh`, `build-release.sh`, `run-release-server.sh`.
- Done when: the prerender test passes, `cargo leptos watch` serves a playable game against `./scripts/dev/server.sh`, and two browsers play together.

### Phase 9: verification
- wasm replay test under wasmtime (`wasm32-wasip1`), growth statistics script, manual verification list, bug-fix ledger walk-through, documentation pass.
- Done when: every automated suite passes and the manual checks are recorded in the PR.
