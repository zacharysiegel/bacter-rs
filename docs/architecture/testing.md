# Testing

## Simulation unit tests
- Location: `shared`, inline `#[cfg(test)] mod tests`.
- `CellOccupancy`: insert, remove, neighbours, re-tighten, row-major order, value equality of equal sets after re-tightening; `from_tight_bitmap` rejects a wrong length or a non-tight box.
- Exposed and adjacent enumeration, including adjacent multiplicity k.
- Border rule: rectangle and ellipse, offset and shrunk worlds, both birth and death.
- Collision: `|dx| <= 6 && |dy| <= 6` inclusive across unaligned lattices.
- Growth state selection (XOR), frozen and immortal skips.
- Every ability: activation, durations, cooldown expiry ticks, key priority, shot miss keeps the carried ability Ready, pop applies once per target, neutralize protection, teammate immunity, self acid, toxin owner skip.
- Shot target selection with wraparound cases and the zero-aim no-op.
- Spore launch: progressive centroid directions, a zero-offset spore removed without a projectile, the last spore of an organism with no enclosed cells.
- Projectile flight: total distance equals nominal speed times duration (24 moves of 10752 subpixels for spores, 21 of 8960 for shots).
- Ability predicates: a Compress or Freeze caster's Active timer is neither Extended nor Immortal.
- Kill credit: last removal wins; suicide; departed hitter; teammate and neutralized hits do not record.
- Rounds: every transition, cancel, force spawn cap, survivor win, simultaneous deaths with no winner, world restore.
- Spawn: hazards, attempt limit.
- Team balance and smallest-team candidates.
- Leaderboard ordering per mode.
- `Pcg32` against the published PCG32 reference vectors and a committed golden sequence; `from_parts` rejects an even increment.
- Growth chance tables: golden digest.

## Determinism tests
- Same seed and bundles stepped twice in process give identical checksums every tick for 10,000 ticks with 16 scripted players using every ability.
- Golden checksum sequence for a committed scripted game (`shared/tests/fixtures/scripted_game.replay` + `.checksums`).
- Snapshot round trip: at tick t, `GameStateSerialOut::from`, encode, decode, `GameState::try_from`; continue both copies, checksums equal.
- `shared/tests/forbidden_operations.rs`: scans `shared/src` (outside `render/`, `play/`) for std float calls (`.ln(`, `.sin(`, `f64::powf` and the rest of [forbidden operations](./determinism.md#forbidden-operations)), `mul_add`, `HashMap`, `HashSet`; `libm::` paths are allowed. The same test fails on `bitcode` named anywhere in `shared/src` outside `protocol/`.
- `scripts/test/check-wasm.sh`: `cargo check --target wasm32-unknown-unknown` for `shared` (with and without `render`) and `web` (`hydrate`).
- `scripts/test/test-replay-wasm.sh`: builds `shared/tests/replay.rs` for `wasm32-wasip1` and runs it under wasmtime, comparing the committed checksum files; covers 32-bit `usize` arithmetic and wasm float behaviour without a browser.
- No `#[wasm_bindgen_test]` re-run of simulation tests: the simulation has no browser-specific code.

## Replay tests
- Replay file: the `u16` version prefix, then a stream of length-prefixed bitcode records, one `ReplayHeaderSerialOut { settings: GameSettingsSerialOut, seed: u64 }` then one `InputBundleSerialOut` per tick ([wire types](./protocol.md#wire-types)), written by the server capture option ([game task](./server.md#game-task)); `ReplayLog { header: ReplayHeader, input_bundles: Vec<InputBundle> }` is the decoded and converted form.
- A file whose version prefix differs from `PROTOCOL_VERSION` gives `ReplayReadError::ProtocolVersionMismatch` ([wire types](./protocol.md#wire-types)).
- Replaying: `GameState::new(settings, seed)` (no members, tick 0), then every bundle in order; creators join through bundle 1 like everyone else ([lifecycle](./server.md#lifecycle)).
- `shared/tests/replay.rs`: every fixture in `shared/tests/fixtures/*.replay` re-run, compared with its committed checksum file.
- Fixtures and checksums regenerated deliberately after a rule change or any wire-type change (`PROTOCOL_VERSION` bump):
  - `scripted_game.replay`: rebuilt from the scripted players of [determinism tests](#determinism-tests) by the `#[ignore]` test `regenerate_scripted_game_fixture` in `shared/tests/replay.rs`;
  - every `.checksums`: rewritten from its `.replay` with `protocol_dump replay --checksums`;
  - both through `scripts/test/regenerate-replay-fixtures.sh` ([scripts](./dev-workflow.md#scripts));
  - a fixture captured from a server game cannot be rebuilt after a wire-type change; it is captured again or deleted.

## Protocol tests
- Round trip for every `MessageSerialIn` and `MessageSerialOut` variant.
- Round trip (encode, decode, equal) for every wire type ([wire types](./protocol.md#wire-types)), `ReplayHeaderSerialOut` included.
- Model to wire to model equality for every `SerialOut` and `Serial` type with a model (`Model::try_from(<Model>SerialOut::from(&model)) == model`, likewise for `Serial`), over scripted game states with every ability phase, round phase and projectile kind.
- `SerialIn` types (no `From<&Model>`): wire to model acceptance and rejection, covered by the `TryFrom` rejection and semantic validation bullets below.
- Equal cell sets built in different insertion orders give byte-equal encodings of `CellOccupancySerialOut::from`.
- `TryFrom` rejections: non-tight or wrong-length `CellOccupancySerialOut`, unordered or duplicate members, `member_id` not below `next_member_id`, even `Pcg32SerialOut` increment, unknown press bits, mode-inconsistent team, loadout or round.
- Malformed input: truncation, bit flips, random bytes, appended byte over many seeds; decode returns `Err` or a value, never panics; accepted values go through `TryFrom`.
- Semantic validation: names, titles, passwords at and past limits, settings ranges and cross-field rules, non-increasing `client_tick`, `aim` without a shot press.
- Size tests, upper budgets only (budgets accepted by the owner): `InputBundleSerialOut` at most 128 B for 8 players and 256 B for 16 players with every player moving and pressing; snapshot at most 32 KB at the player cap ([snapshot](./protocol.md#snapshot)).

## Server integration tests
- Location: `server/tests/`.
- In-process server bound to `127.0.0.1:0` with `ServerRegistries::leaked()` ([registries](./server.md#registries)), so tests in one binary run in parallel without sharing games or titles; clients over tokio-tungstenite; shared helper `tests/helpers/test_server.rs` (used by both files below).
- `game_lifecycle.rs`: create plus join in one message; second client joins and receives snapshot then bundles; both clients' checksums agree with the server's; spectate; die and respawn; leave; leave then join another game at once (no frame of the first game after `LeftGame`); disconnect; a client that stops reading is removed from the simulation (`MemberEvent::Left`) while others continue; two joins with one name or for the last slot in one tick (one admitted); empty-game removal after the grace period (`ServerConfig.empty_game_grace_period` set to 200 ms); rejoin during grace.
- `protocol_errors.rs`: wrong protocol version; text frame; oversize frame; garbage bytes close only that connection; wrong password; name taken; game full; team unbalanced; rate limits; disallowed `Origin`.
- Clamp: an input jump beyond the budget yields `CursorCorrected` and a clamped bundle cursor.

## Growth statistics against the original rules
- `shared/tests/growth_statistics.rs` (`#[ignore]`, run by script): one organism in a 100000 px world, cursor still or moving at 2.975 px/tick along x, 3000 ticks, first 200 discarded, 8 seeds.
- Reference means: the original growth rules (`Org.js`, master) measured once with a throwaway model (single organism, no border, no opponents, float cursor, birth sites with multiplicity, reset to one cell at the cursor on death); committed as constants in the test:

| State    | Cursor | Cells | Changes/tick |
| -------- | ------ | ----- | ------------ |
| Default  | still  | 64.0  | 6.2          |
| Default  | moving | 42.5  | 7.7          |
| Compress | still  | 37.8  | 0.8          |
| Compress | moving | 23.0  | 4.4          |
| Extend   | still  | 124.8 | 7.8          |
| Extend   | moving | 81.2  | 10.4         |

- Assert mean cell count within 10% and mean changes per tick within 15%; the cursor is integer-rounded in Rust and was float in the reference model.

## Client and server unit tests
- `play/client_session.rs`: snapshot then bundles; checksum mismatch starts a resync and discards bundles until the snapshot; an undecodable or unconvertible bundle or snapshot starts a resync; backlog trigger; response timeout re-send; resync limit gives `SynchronisationLost`; no `Input` after a snapshot alone.
- `play/crosshair.rs`: speed and diagonal scaling; a clamp burst over several ticks shifts the crosshair once (history rebase); correction past the history snaps.
- `play/game_key.rs`: keydown edges, repeat ignored, edges discarded without an organism.
- `play/frame_pacing.rs`: dt clamp, accumulator tick count and per-frame limit, alpha clamp.
- `play/message_text.rs`: every message per mode, phase and own state, singular and plural waiting texts.
- `tutorial/tutorial.rs`: task progression, ability mask per task, bot minimum distance, per-organism replacement on death.
- `socket/rate_limit.rs`: refill, burst, abuse detection.
- `game/cursor_clamp.rs`: within budget, beyond budget along the same direction, budget cap, reset on spawn, i64 extremes.
- `game/pending_admissions.rs` and `game/membership.rs`: names, slots, team sizes and member ids counted across pending admissions.
- `lobby/lobby_broadcast.rs`: revision on summary and client-count changes, at most one list per second.

## Other tests
- `web/tests/prerender.rs` (ssr): prerendered document contains the canvas root, `/pkg/`, `.wasm`, `.css`, `<title>`.
- `LocalStorageSettingsStore`: `#[wasm_bindgen_test]` (browser-only behaviour), via `scripts/test/test-wasm.sh`.
- Shader pipelines: `#[ignore]` test building every pipeline on a headless device, via `scripts/test/test-shaders.sh`.

## Manual verification
- Visual parity with the original screens, skins, HUD, leaderboard, messages, title and tutorial.
- WebGPU and `?renderer=webgl2` in Chrome and Safari; device pixel ratio 1 and 2.
- Feel: crosshair speed, spore and shot flight, cooldown arcs.
- Two browsers in one game on this machine; throttled network in devtools (latency, packet loss) for clamp and resync behaviour.
