# Dev workflow

## Script rules
- `#!/usr/bin/env bash` and `set -euo pipefail` in every script.
- A command's status or output is captured into a named variable before any branch on it (`installed_cargo_leptos=$(command -v cargo-leptos || true); if test -z "${installed_cargo_leptos}"; then`); never a command inside an `if` or `while` condition.
- Scripts adapted from eafora are rewritten to these rules, not copied as is; eafora-specific parts are removed.
- The default branch is `master` (owner decision; [`docs/conventions/`](#docsconventions)). Git scripts read it once: `default_ref=$(git symbolic-ref --short refs/remotes/origin/HEAD)` (`origin/master`), then `DEFAULT_BRANCH="${default_ref#origin/}"`.

## Scripts
- `setup.sh`: check `cargo`, `cargo-leptos`, `jq`; `rustup target add wasm32-unknown-unknown wasm32-wasip1`; optional checks for `wasm-pack`, `chromedriver` and `wasmtime`; create `.env` from `template.env` when absent. No secrets.
- `scripts/dev/server.sh`: `cargo run -p server` with `.env`.
- `scripts/dev/web.sh`: `cd web && cargo leptos watch`.
- `scripts/run/run-release-server.sh`: loads `.env`, checks that `BACTER_BIND_ADDRESS`, `BACTER_PORT`, `BACTER_SITE_ROOT` and `BACTER_ALLOWED_ORIGINS` are set and that the site root holds `index.html`; checks that `BACTER_TLS_CERTIFICATE_PATH` and `BACTER_TLS_PRIVATE_KEY_PATH` are both set or both unset and, when set, that both files are readable; then runs `target/release/server`.
- `scripts/build/build-site.sh` (adapted from eafora): refuse while anything listens on the web `site-addr` port (3011); `LEPTOS_HASH_FILES=true cargo leptos build --release`; `target/release/web prerender`; `jq` rewrite of `target/site/game-server.json`; `verify-site-tree.sh`.
- `scripts/build/verify-site-tree.sh`: generic part of eafora's (index exists, every `/pkg/*` reference on disk and hashed, at least one asset).
- `scripts/build/build-release.sh`: `build-site.sh`, then `cargo build --release -p server`.
- `scripts/test/check-wasm.sh`: [determinism tests](./testing.md#determinism-tests).
- `scripts/test/test-wasm.sh`: `wasm-pack test --headless --chrome` from `web/` with `--features hydrate` (the `LocalStorageSettingsStore` tests, [other tests](./testing.md#other-tests)).
- `scripts/test/test-replay-wasm.sh`: [determinism tests](./testing.md#determinism-tests).
- `scripts/test/test-shaders.sh`, `scripts/test/test-growth-statistics.sh`: run the `#[ignore]` tests of [other tests](./testing.md#other-tests) and [growth statistics against the original rules](./testing.md#growth-statistics-against-the-original-rules).
- `scripts/test/regenerate-replay-fixtures.sh`: runs `regenerate_scripted_game_fixture`, then `protocol_dump replay --checksums` for every fixture ([replay tests](./testing.md#replay-tests)).
- `scripts/git/branch-init.sh`, `pr-integrate.sh`, `cleanup-merged.sh`: adapted from eafora; `pr-integrate.sh` has no `--budget` mode (bacter-rs has no site budget script).

## Running locally
```sh
./scripts/dev/server.sh
```
```sh
./scripts/dev/web.sh
```
- Browse `http://127.0.0.1:3011`; the page connects to `ws://127.0.0.1:3100/ws` from the committed `game-server.json`.

## Production
```sh
./scripts/build/build-release.sh
```
```sh
./scripts/run/run-release-server.sh
```
- `.env` on the host sets `BACTER_SITE_ROOT=target/site`, `BACTER_ALLOWED_ORIGINS=https://<host>`, `BACTER_BIND_ADDRESS=0.0.0.0` and `BACTER_PORT`:
  - `443` with the TLS variables (direct, or Cloudflare Full (strict));
  - `80` for Cloudflare Flexible;
  - or another port with a Cloudflare Origin Rule mapping a Cloudflare-proxied port to it.
- Ports below 1024 on Linux need `CAP_NET_BIND_SERVICE` or root for the server process.
- TLS: either rustls in the server (`BACTER_TLS_CERTIFICATE_PATH`, `BACTER_TLS_PRIVATE_KEY_PATH`, [process](./server.md#process)), or Cloudflare in front: Full (strict) keeps the TLS variables (origin TLS), Flexible leaves them unset (plain HTTP to the origin). Chosen at deploy time; the browser sees `https:`/`wss:` either way, and `game-server.json` (null URL, [game server URL](./client-web.md#game-server-url)) follows the page's scheme.
- One origin serves `index.html`, `/pkg/*`, `/fonts/*`, `/game-server.json` and `/ws`.

## `docs/conventions/`
- Adapted from eafora:
  - `README.md`: index, renamed to bacter-rs; eafora's Spec Kit references (per-feature `plan.md`/`tasks.md`, "Relationship to the constitution") are dropped (no constitution, below).
  - `logging.md`, `conditional-compilation.md`, `shading.md`: as is.
  - `types.md`: hierarchical naming, `Kind` suffix, `code()`/`as_str()`, `TryFrom<&str>` never `FromStr`, `SerialIn`/`SerialOut`/`Serial` and their `From`/`TryFrom` conversions extended to WebSocket messages and replay files ([wire types](./protocol.md#wire-types)): every type on the wire or in a replay file has a dedicated wire type, and only wire types derive bitcode; the body states that wire types live in `shared/src/protocol/` (not `<feature>_model.rs`), with conversions immediately after the wire type in that file; the receiving side converts with `TryFrom<<Model>SerialOut>` or `TryFrom<<Model>Serial>`; replay files are server output and use `SerialOut` types, the header included; Entity/Projection and `query_scalar!` deferred until a database exists.
    - A "Where bacter-rs diverges from Singularity" section: bitcode in place of serde and rmp-serde.
- No Spec Kit and no constitution (owner decision): no `.specify/`, no `specs/NNN-slug/`; governance lives in the two docs below.
- New:
  - `git-workflow.md`, carrying eafora's constitution rules:
    - default branch `master`, locally and on GitHub;
    - one short-lived branch per body of work; serial phases stacked, each PR targeting its parent branch;
    - first commit an empty marker `>>> branch: <name>`, created by `scripts/git/branch-init.sh`;
    - one-line commit messages; staging by explicit path; no attribution lines;
    - every commit pushed at once;
    - PR via `gh pr create --assignee @me`; description as a release note (problem and solution, no test plan);
    - integration by local rebase through `scripts/git/pr-integrate.sh`, never the GitHub rebase button; `--force-with-lease` only;
    - merged branches removed with `scripts/git/cleanup-merged.sh`;
    - script registry: each `scripts/git/` script named with the rule it implements.
  - `testing.md`: test-first for simulation rules, determinism, protocol codec and validation, admission and clamp logic; UI views and shaders exempt; Singularity-parity defaults unless a divergence is recorded in the relevant convention doc.

## `CLAUDE.md` for bacter-rs
- Project: Rust rewrite of Bacter; reference `/Users/singularity/bacter` branch `master`; models `/Users/singularity/eafora` and `/Users/singularity/singularity`.
- Runs entirely on this machine; never require a hosted server.
- Conventions index pointing at `docs/conventions/` (including `git-workflow.md` and `testing.md`); architecture at `docs/architecture/`.
- Determinism contract summary ([determinism rules](./determinism.md#determinism-rules)) and the rule that every simulation change or wire-type change keeps replay fixtures passing or regenerates the fixtures and checksums deliberately ([replay tests](./testing.md#replay-tests)).
- Wire types ([wire types](./protocol.md#wire-types)): only the `...Serial*` types in `shared/src/protocol/` derive bitcode; changing one bumps `PROTOCOL_VERSION`.
- Invoke wrappers in `scripts/`, never the raw tool, including in commands written for the owner.
- `mod.rs` holds only `pub mod x;` + `pub use x::*;` pairs and doc comments ([module rules](./overview.md#module-rules)); file order rule; poisoned locks unwrapped; handle versus on naming for input handlers.
- New dependencies discussed with the owner first.
- Owner preferences pointer to `~/.claude/CLAUDE.md`.
