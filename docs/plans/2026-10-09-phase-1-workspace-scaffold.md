# Phase 1: workspace scaffold implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** a Cargo workspace whose four crates (`shared`, `server`, `web`, `tools/protocol_dump`) compile as empty shells with their final dependencies, feature flags and per-target tables, plus the root configuration, the first scripts, `docs/conventions/` and the project `CLAUDE.md`.

**Architecture:** the layout, manifests and rules come from `docs/architecture/overview.md` (workspace layout, root `Cargo.toml`, the four crate sections, module rules) and `docs/architecture/dev-workflow.md` (script rules, scripts, `docs/conventions/`, `CLAUDE.md`). No crate has code yet beyond `fn main() {}` and crate attributes; every later phase adds modules to these shells without touching their manifests' feature or target structure.

**Tech stack:** Rust 1.95 (edition 2024, resolver 3), bitcode, libm, minimer, wgpu 30, Leptos 0.8 with cargo-leptos, actix-web 4.13 with rustls 0.23, actix-ws, actix-files, tokio, bash scripts.

---

## Ground rules for the executor

- Repository `/Users/singularity/bacter-rs`, branch `workspace-scaffold`. Run `git -C /Users/singularity/bacter-rs branch --show-current` first; stop if it does not print `workspace-scaffold`.
- Every command runs from the repository root `/Users/singularity/bacter-rs`.
- Never switch branches, push, rebase or amend. Stage by explicit path, run `git status` before each commit, one-line commit messages without attribution.
- Never run `cargo clippy`.
- Phase 1 has no testable logic: every task verifies with exact commands and their expected output in place of failing tests (writing-plans rule for scaffolding).

## Decisions taken where the design is silent or contradictory

- `Cargo.lock` is seeded from `/Users/singularity/eafora/Cargo.lock` before the first resolve. A fresh resolve today picks wasm-bindgen 0.2.129, web-sys 0.3.106 and wgpu 30.0.1 (which needs web-sys `^0.3.104`); `cargo update -p wasm-bindgen --precise 0.2.122` then fails, because the wasm-bindgen family (wasm-bindgen, js-sys, web-sys, wasm-bindgen-futures, wasm-bindgen-test) pins each other with exact `=` requirements and `--precise` moves only one package. Seeding keeps every crate present in the eafora lockfile at its eafora version (the design's "take that minor") and resolves the rest fresh; the design's `--precise 0.2.122` command then runs as a no-op confirmation. Verified in a trial workspace on 2026-10-09.
- actix-files resolves to `0.7.0`, so its wildcard is `"0.7.*"` (the design's "resolve" row). bitcode resolves to `0.6.9` under the owner's `"0.6.*"`.
- Manifests carry their complete final dependency lists now (from overview.md), so the lockfile is resolved and the wasm-bindgen pin established once, and later phases only add code.
- `shared`'s `render` feature also enables `dep:web-sys`: overview.md lists web-sys as "optional, enabled by `render`" in the wasm32 table but omits it from the feature list.
- `tools/protocol_dump` depends on `log` as well as shared, clap, minimer and env_logger (the version table lists log as used by all crates). `web` does not depend on minimer directly: the `web` crate section lists its dependencies without it, and that per-crate list wins over the table.
- `#![recursion_limit = "512"]` carries a one-line comment rather than eafora's block comments, which `cargo fmt` re-indents at the top of a file.
- Scripts in this phase are the ones whose target already exists: `scripts/test/check-wasm.sh`, `scripts/dev/server.sh` (the `server` binary exists, as `fn main() {}`), and the three git scripts. The others arrive with their targets: `scripts/dev/web.sh` needs the ssr `serve()` and `style/main.scss` (shell, phase 8); the build and run scripts are phase 8; `test-wasm.sh`, `test-shaders.sh`, `test-replay-wasm.sh`, `test-growth-statistics.sh` and `regenerate-replay-fixtures.sh` arrive with the tests they run.
- The three git scripts share their precondition checks through a sourced `scripts/git/git-preconditions.sh` (not run directly), rather than repeating them in each script.
- `origin/HEAD` is not set in this repository (it was created with `git remote add`, not cloned). The git scripts read the default branch from it as the design requires, and exit with the hint `git remote set-head origin --auto` when it is missing; this plan does not change the repository's remote configuration.
- `.gitignore` already holds `/target/` and `.env` (overview.md: "target/, .env") and is not modified.

## File structure

- Create `rust-toolchain.toml`: pins channel 1.95.
- Create `rustfmt.toml`: formatter settings, copied from eafora.
- Create `Cargo.toml`: virtual workspace, members, `[workspace.dependencies]`, `wasm-release` profile.
- Create `Cargo.lock`: resolved lockfile, seeded from eafora's.
- Create `shared/Cargo.toml`, `shared/src/lib.rs`: library shell; feature `render`; per-target wgpu, raw-window-handle and web-sys tables.
- Create `server/Cargo.toml`, `server/src/lib.rs`, `server/src/main.rs`: library plus `server` binary shell.
- Create `web/Cargo.toml`, `web/src/lib.rs`, `web/src/main.rs`: cdylib/rlib plus binary shell; features `hydrate` and `ssr`; `[package.metadata.leptos]`.
- Create `tools/protocol_dump/Cargo.toml`, `tools/protocol_dump/src/main.rs`: binary shell.
- Create `scripts/test/check-wasm.sh`: wasm32 type-check of shared (with and without `render`) and web (`hydrate`).
- Create `template.env`: non-secret server settings.
- Create `setup.sh`: prerequisite checks, wasm targets, `.env` creation.
- Create `scripts/dev/server.sh`: runs the game server with `.env`.
- Create `scripts/git/git-preconditions.sh`: precondition functions sourced by the git scripts.
- Create `scripts/git/branch-init.sh`: branch with marker commit, pushed.
- Create `scripts/git/cleanup-merged.sh`: removes merged branches from origin and locally.
- Create `scripts/git/pr-integrate.sh`: integration by local rebase.
- Create `docs/conventions/README.md`, `logging.md`, `conditional-compilation.md`, `shading.md`, `types.md`, `git-workflow.md`, `testing.md`: coding conventions.
- Create `CLAUDE.md`: project guidance for agents.

---

### Task 1: Toolchain and formatter configuration

**Files:**
- Create: `rust-toolchain.toml`
- Create: `rustfmt.toml`

- [ ] **Step 1: Write `rust-toolchain.toml`**

```toml
[toolchain]
channel = "1.95"
```

- [ ] **Step 2: Copy `rustfmt.toml` from eafora**

Run: `cp /Users/singularity/eafora/rustfmt.toml ./rustfmt.toml`

Its content is:

```toml
edition = "2024"
max_width = 120
chain_width = 100
remove_nested_parens = true
```

- [ ] **Step 3: Verify**

Run: `rustup show active-toolchain`
Expected: `1.95-aarch64-apple-darwin (overridden by '/Users/singularity/bacter-rs/rust-toolchain.toml')`

Run: `diff /Users/singularity/eafora/rustfmt.toml ./rustfmt.toml`
Expected: no output, exit status 0.

- [ ] **Step 4: Commit**

```sh
git status
git add ./rust-toolchain.toml ./rustfmt.toml
git commit -m "toolchain and rustfmt configuration"
```

`git status` before the commit shows only these two files as new.

### Task 2: Workspace manifest and empty crate shells

**Files:**
- Create: `Cargo.toml`
- Create: `Cargo.lock`
- Create: `shared/Cargo.toml`, `shared/src/lib.rs`
- Create: `server/Cargo.toml`, `server/src/lib.rs`, `server/src/main.rs`
- Create: `web/Cargo.toml`, `web/src/lib.rs`, `web/src/main.rs`
- Create: `tools/protocol_dump/Cargo.toml`, `tools/protocol_dump/src/main.rs`

All four members are created before the first build so the lockfile is resolved once against the eafora seed (see the decisions above).

- [ ] **Step 1: Write the root `Cargo.toml`**

```toml
[workspace]
resolver = "3"
members = [
    "shared",
    "server",
    "web",
    "tools/protocol_dump",
]

[workspace.package]
edition = "2024"

[workspace.dependencies]
shared = { path = "shared" }

# simulation and protocol
bitcode = "0.6.*"
libm = "0.2.*"
minimer = "2.2.*"
log = "0.4.*"

# renderer
wgpu = { version = "30.0.*", default-features = false }
raw-window-handle = "0.6.*"
bytemuck = { version = "1.25.*", features = ["derive"] }
web-sys = "0.3.*"

# game server
actix-web = "4.13.*"
rustls = "0.23.*"
actix-ws = "0.4.*"
actix-files = "0.7.*"
tokio = { version = "1.52.*", default-features = false }
futures-util = "0.3.*"
bytes = "1.11.*"
dashmap = "6.1.*"
bcrypt = "0.17.*"
getrandom = "0.4.*"
env_logger = "0.11.*"
dotenvy = "0.15.*"
tokio-tungstenite = "0.26.*"

# tools
clap = "4.6.*"

# web client
leptos = "0.8.*"
leptos_meta = "0.8.*"
leptos_axum = "0.8.*"
axum = "0.8.*"
any_spawner = { version = "0.3.*", features = ["tokio"] }
wasm-bindgen = "0.2.*"
wasm-bindgen-futures = "0.4.*"
js-sys = "0.3.*"
console_log = "0.2.*"
console_error_panic_hook = "0.1.*"
wasm-bindgen-test = "0.3.*"

[profile.wasm-release]
inherits = "release"
opt-level = "z"
lto = true
codegen-units = 1
strip = true
```

- [ ] **Step 2: Write the `shared` crate shell**

`shared/Cargo.toml`:

```toml
[package]
name = "shared"
version = "0.0.0"
edition.workspace = true
publish = false

[lib]

[dependencies]
bitcode = { workspace = true }
libm = { workspace = true }
minimer = { workspace = true }
log = { workspace = true }
bytemuck = { workspace = true, optional = true }

[features]
render = [
    "dep:wgpu",
    "dep:raw-window-handle",
    "dep:bytemuck",
    "dep:web-sys",
]

# wasm32 has neither Metal nor Vulkan, nor a raw window handle to render into.
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
wgpu = { workspace = true, optional = true, features = ["metal", "vulkan", "wgsl"] }
raw-window-handle = { workspace = true, optional = true }

# Other targets have no browser WebGPU, WebGL2 or canvas element.
[target.'cfg(target_arch = "wasm32")'.dependencies]
wgpu = { workspace = true, optional = true, features = ["webgpu", "webgl", "wgsl"] }
web-sys = { workspace = true, optional = true, features = ["HtmlCanvasElement"] }
```

`shared/src/lib.rs`:

```rust
//! Everything the server and the clients must agree on, plus platform-independent client logic.
```

- [ ] **Step 3: Write the `server` crate shell**

`server/Cargo.toml`:

```toml
[package]
name = "server"
version = "0.0.0"
edition.workspace = true
publish = false

[lib]

[[bin]]
name = "server"
path = "src/main.rs"

[dependencies]
shared = { workspace = true }
actix-web = { workspace = true, features = ["rustls-0_23"] }
rustls = { workspace = true }
actix-ws = { workspace = true }
actix-files = { workspace = true }
tokio = { workspace = true, features = ["full"] }
futures-util = { workspace = true }
bytes = { workspace = true }
dashmap = { workspace = true }
bcrypt = { workspace = true }
getrandom = { workspace = true }
log = { workspace = true }
env_logger = { workspace = true }
dotenvy = { workspace = true }
minimer = { workspace = true }

[dev-dependencies]
tokio-tungstenite = { workspace = true }
```

`server/src/lib.rs`:

```rust
//! The game server: WebSocket endpoint, game registry, one task per game, and static site serving.
```

`server/src/main.rs`:

```rust
fn main() {}
```

- [ ] **Step 4: Write the `web` crate shell**

`web/Cargo.toml`:

```toml
[package]
name = "web"
version = "0.0.0"
edition.workspace = true
publish = false

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
shared = { workspace = true, features = ["render"] }
leptos = { workspace = true }
leptos_meta = { workspace = true }
log = { workspace = true }
wasm-bindgen = { workspace = true, optional = true }
wasm-bindgen-futures = { workspace = true, optional = true }
js-sys = { workspace = true, optional = true }
web-sys = { workspace = true, optional = true, features = [
    "Window",
    "Document",
    "Location",
    "Storage",
    "Performance",
    "HtmlCanvasElement",
    "HtmlElement",
    "Element",
    "DomRect",
    "EventTarget",
    "Event",
    "KeyboardEvent",
    "MouseEvent",
    "FocusEvent",
    "WebSocket",
    "BinaryType",
    "MessageEvent",
    "CloseEvent",
    "ErrorEvent",
    "Response",
    "RequestInit",
] }
console_log = { workspace = true, optional = true }
console_error_panic_hook = { workspace = true, optional = true }
leptos_axum = { workspace = true, optional = true }
axum = { workspace = true, optional = true }
any_spawner = { workspace = true, optional = true }
tokio = { workspace = true, optional = true }

# Other targets are not downloaded, so they keep debug and trace records in release builds.
[target.'cfg(target_arch = "wasm32")'.dependencies]
log = { workspace = true, features = ["release_max_level_info"] }

# Other targets have no browser to run #[wasm_bindgen_test] cases in.
[target.'cfg(target_arch = "wasm32")'.dev-dependencies]
wasm-bindgen-test = { workspace = true }

[features]
hydrate = [
    "leptos/hydrate",
    "dep:wasm-bindgen",
    "dep:wasm-bindgen-futures",
    "dep:js-sys",
    "dep:web-sys",
    "dep:console_log",
    "dep:console_error_panic_hook",
]
ssr = [
    "leptos/ssr",
    "leptos_meta/ssr",
    "dep:leptos_axum",
    "dep:axum",
    "dep:any_spawner",
    "tokio/rt",
    "tokio/rt-multi-thread",
    "tokio/net",
    "tokio/macros",
]

[package.metadata.leptos]
output-name = "bacter"
site-root = "target/site"
site-pkg-dir = "pkg"
style-file = "style/main.scss"
assets-dir = "static"
site-addr = "127.0.0.1:3011"
reload-port = 3012
browserquery = "defaults"
bin-features = ["ssr"]
lib-features = ["hydrate"]
bin-default-features = false
lib-default-features = false
lib-profile-release = "wasm-release"
```

`web/src/lib.rs`:

```rust
// The release view tree's monomorphized type exceeds rustc's default query depth.
#![recursion_limit = "512"]
```

`web/src/main.rs`:

```rust
// Laying out the lib's release render future exceeds rustc's default query depth.
#![recursion_limit = "512"]

fn main() {}
```

- [ ] **Step 5: Write the `tools/protocol_dump` crate shell**

`tools/protocol_dump/Cargo.toml`:

```toml
[package]
name = "protocol_dump"
version = "0.0.0"
edition.workspace = true
publish = false

[dependencies]
shared = { workspace = true }
clap = { workspace = true }
minimer = { workspace = true }
log = { workspace = true }
env_logger = { workspace = true }
```

`tools/protocol_dump/src/main.rs`:

```rust
fn main() {}
```

- [ ] **Step 6: Seed the lockfile, resolve and build**

Run: `cp /Users/singularity/eafora/Cargo.lock ./Cargo.lock`

Run: `cargo build --workspace`
Expected: cargo prints `Adding` lines for the crates eafora does not use (actix-web, actix-files, actix-ws, bitcode, dashmap, tokio-tungstenite and their dependencies) and a few `Updating`/`Downgrading` lines for shared transitive crates, compiles, and ends with `Finished `dev` profile [unoptimized + debuginfo] target(s) in ...`. No warnings.

Run: `cargo update -p wasm-bindgen --precise 0.2.122`
Expected: `Updating crates.io index` and a `note: pass --verbose ...` line; no `Updating wasm-bindgen` or `Downgrading` line.

Run: `grep -A1 -E '^name = "(wasm-bindgen|web-sys|js-sys|wgpu|leptos|bitcode|actix-files|actix-web)"$' ./Cargo.lock`
Expected (the `--` separators omitted):

```
name = "actix-files"
version = "0.7.0"
name = "actix-web"
version = "4.13.0"
name = "bitcode"
version = "0.6.9"
name = "js-sys"
version = "0.3.99"
name = "leptos"
version = "0.8.20"
name = "wasm-bindgen"
version = "0.2.122"
name = "web-sys"
version = "0.3.99"
name = "wgpu"
version = "30.0.0"
```

- [ ] **Step 7: Verify tests, formatting and feature isolation**

Run: `cargo test --workspace`
Expected: every `test result:` line reads `ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.

Run: `cargo fmt --all --check`
Expected: no output, exit status 0.

Run: `cargo check -p web --features ssr`
Expected: ends with `Finished`.

Run: `cargo tree -p server -e normal -i wgpu`
Expected: `error: package ID specification `wgpu` did not match any packages` (the server's own dependency graph never links wgpu).

Run: `cargo tree -p shared -e normal --depth 1`
Expected:

```
shared v0.0.0 (/Users/singularity/bacter-rs/shared)
├── bitcode v0.6.9
├── libm v0.2.16
├── log v0.4.30
└── minimer v2.2.0
```

- [ ] **Step 8: Commit**

```sh
git status
git add ./Cargo.toml ./Cargo.lock ./shared/Cargo.toml ./shared/src/lib.rs ./server/Cargo.toml ./server/src/lib.rs ./server/src/main.rs ./web/Cargo.toml ./web/src/lib.rs ./web/src/main.rs ./tools/protocol_dump/Cargo.toml ./tools/protocol_dump/src/main.rs
git commit -m "workspace and empty crate shells"
```

`git status` before the commit shows `target/` ignored and only the listed files as new.

### Task 3: wasm32 check script

**Files:**
- Create: `scripts/test/check-wasm.sh`

- [ ] **Step 1: Write `scripts/test/check-wasm.sh`**

```bash
#!/usr/bin/env bash
# Type-checks the wasm32 builds: shared with and without the renderer, and the web client's hydrate build.

set -euo pipefail

readonly WASM_TARGET="wasm32-unknown-unknown"

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

cargo check -p shared --target "${WASM_TARGET}"
cargo check -p shared --target "${WASM_TARGET}" --features render
cargo check -p web --target "${WASM_TARGET}" --features hydrate
```

Run: `chmod +x ./scripts/test/check-wasm.sh`

- [ ] **Step 2: Verify**

Run: `./scripts/test/check-wasm.sh; echo "exit=$?"`
Expected: three `Finished `dev` profile` lines (preceded on the first run by `Checking` lines for shared, wgpu and web), then `exit=0`.

- [ ] **Step 3: Commit**

```sh
git status
git add ./scripts/test/check-wasm.sh
git commit -m "wasm32 check script"
```

### Task 4: Environment template and setup script

**Files:**
- Create: `template.env`
- Create: `setup.sh`

- [ ] **Step 1: Write `template.env`**

Variables and defaults from `docs/architecture/server.md` (process); optional variables stay commented out so they are unset.

```sh
# Non-secret server settings. setup.sh copies this file to .env when .env is absent.

BACTER_BIND_ADDRESS=127.0.0.1
BACTER_PORT=3100

# Comma-separated page origins allowed to open /ws.
BACTER_ALLOWED_ORIGINS=http://127.0.0.1:3011,http://localhost:3011

# Unset: the server serves no static files.
# BACTER_SITE_ROOT=target/site

# Unset: no replay capture.
# BACTER_REPLAY_CAPTURE_DIRECTORY=/tmp/bacter/replays

# PEM files; set both for TLS, or neither for plain HTTP.
# BACTER_TLS_CERTIFICATE_PATH=
# BACTER_TLS_PRIVATE_KEY_PATH=

RUST_LOG=info
```

- [ ] **Step 2: Write `setup.sh`**

Follows the script rules of `docs/architecture/dev-workflow.md`: every command's output is captured into a named variable before any branch on it.

```bash
#!/usr/bin/env bash
# One-time setup of a development machine; safe to re-run.

set -euo pipefail

readonly MISSING_PROGRAM_EXIT_STATUS=1

function require_program {
    local program_name="$1"
    local install_hint="$2"

    local program_path
    program_path=$(command -v "${program_name}" || true)

    if test -z "${program_path}"; then
        echo "error: the \`${program_name}\` program is required; install it with: ${install_hint}" >&2
        exit "${MISSING_PROGRAM_EXIT_STATUS}"
    fi
}

function recommend_program {
    local program_name="$1"
    local needed_by="$2"
    local install_hint="$3"

    local program_path
    program_path=$(command -v "${program_name}" || true)

    if test -z "${program_path}"; then
        echo "note: \`${program_name}\` is not installed; ${needed_by} needs it; install it with: ${install_hint}"
    fi
}

function create_environment_file {
    if test -f ./.env; then
        echo "keeping the existing ./.env"
        return 0
    fi

    cp ./template.env ./.env
    echo "created ./.env from ./template.env"
}

function main {
    local repository_root
    repository_root=$(git rev-parse --show-toplevel)
    cd "${repository_root}"

    require_program "cargo" "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    require_program "cargo-leptos" "cargo install --locked cargo-leptos"
    require_program "jq" "brew install jq"

    rustup target add wasm32-unknown-unknown wasm32-wasip1

    recommend_program "wasm-pack" "the headless browser tests" "cargo install wasm-pack"
    recommend_program "chromedriver" "the headless browser tests" "a build matching the installed Chrome major version, on PATH"
    recommend_program "wasmtime" "the wasm32-wasip1 replay test" "brew install wasmtime"

    create_environment_file

    echo "setup complete"
}

main
```

Run: `chmod +x ./setup.sh`

- [ ] **Step 3: Verify**

Run: `test -f ./.env && mv ./.env /tmp/bacter-rs-env-before-setup; ./setup.sh; echo "exit=$?"`
Expected: an `info: component rust-std for target ...` line (up to date, or an install) for each of the two targets, a `note:` line for each of wasm-pack, chromedriver and wasmtime that is missing (on this machine: wasmtime only), `created ./.env from ./template.env`, `setup complete`, `exit=0`.

Run: `./setup.sh | tail -2`
Expected: rustup's two `info:` lines (stderr), then on stdout:

```
keeping the existing ./.env
setup complete
```

Run: `diff ./template.env ./.env && git status --short`
Expected: no diff output; `git status --short` lists `?? setup.sh` and `?? template.env` and not `.env` (ignored).

Run: `test -f /tmp/bacter-rs-env-before-setup && mv /tmp/bacter-rs-env-before-setup ./.env; true`
Expected: no output (restores a `.env` that existed before the step, if any).

- [ ] **Step 4: Commit**

```sh
git status
git add ./template.env ./setup.sh
git commit -m "setup script and environment template"
```

### Task 5: Dev server script

**Files:**
- Create: `scripts/dev/server.sh`

- [ ] **Step 1: Write `scripts/dev/server.sh`**

The server loads `.env` itself through dotenvy from its working directory (phase 5); the script runs it from the repository root and refuses when `.env` is missing.

```bash
#!/usr/bin/env bash
# Runs the game server from the repository root, where it reads ./.env.

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

if test ! -f ./.env; then
    echo "error: ./.env is missing; run ./setup.sh first" >&2
    exit 1
fi

exec cargo run -p server
```

Run: `chmod +x ./scripts/dev/server.sh`

- [ ] **Step 2: Verify**

Run: `./scripts/dev/server.sh; echo "exit=$?"`
Expected: cargo compiles `server` (its feature set differs from the workspace build, so shared is rebuilt without `render`), prints ``Running `target/debug/server` ``, then `exit=0` (the shell's `main` returns at once).

Run: `mv ./.env /tmp/bacter-rs-env-moved; ./scripts/dev/server.sh; echo "exit=$?"; mv /tmp/bacter-rs-env-moved ./.env`
Expected:

```
error: ./.env is missing; run ./setup.sh first
exit=1
```

- [ ] **Step 3: Commit**

```sh
git status
git add ./scripts/dev/server.sh
git commit -m "dev server script"
```

### Task 6: Git preconditions and branch-init script

**Files:**
- Create: `scripts/git/git-preconditions.sh`
- Create: `scripts/git/branch-init.sh`

Adapted from eafora's `scripts/git/branch-init.sh`, rewritten to the script rules (statuses captured before branching) with the checks shared by all three git scripts moved into a sourced file.

- [ ] **Step 1: Write `scripts/git/git-preconditions.sh`** (sourced, so no shebang and not executable)

```bash
# Precondition checks shared by the scripts in scripts/git/; sourced, not run.

readonly USAGE_EXIT_STATUS=64

# Exit status of `git ls-remote --exit-code` when no ref matches; other non-zero statuses are failures.
readonly LS_REMOTE_NO_MATCH_STATUS=2

function require_git_repository {
    local git_directory_status=0
    git rev-parse --git-dir > /dev/null 2>&1 || git_directory_status=$?

    if test "${git_directory_status}" -ne 0; then
        echo "error: not inside a git repository" >&2
        exit 1
    fi
}

function require_clean_working_tree {
    local working_tree_status=0
    git diff-index --quiet HEAD -- || working_tree_status=$?

    if test "${working_tree_status}" -ne 0; then
        echo "error: the working tree has uncommitted changes; commit or stash them first" >&2
        exit 1
    fi
}

function read_default_branch {
    local default_ref
    default_ref=$(git symbolic-ref --short refs/remotes/origin/HEAD 2> /dev/null || true)

    if test -z "${default_ref}"; then
        echo "error: origin/HEAD is not set; run: git remote set-head origin --auto" >&2
        exit 1
    fi

    echo "${default_ref#origin/}"
}

function remote_branch_exists {
    local branch_name="$1"

    local ls_remote_status=0
    git ls-remote --exit-code --heads origin "${branch_name}" > /dev/null || ls_remote_status=$?

    if test "${ls_remote_status}" -eq 0; then
        echo "present"
    elif test "${ls_remote_status}" -eq "${LS_REMOTE_NO_MATCH_STATUS}"; then
        echo "absent"
    else
        echo "error: could not list the branches of origin; [status=${ls_remote_status}]" >&2
        exit 1
    fi
}

function local_branch_exists {
    local branch_name="$1"

    local show_ref_status=0
    git show-ref --verify --quiet "refs/heads/${branch_name}" || show_ref_status=$?

    if test "${show_ref_status}" -eq 0; then
        echo "present"
    else
        echo "absent"
    fi
}
```

Callers capture these functions' output with a plain assignment (`default_branch=$(read_default_branch)`, never `local x=$(...)`), so an `exit 1` inside the command substitution fails the assignment and `set -e` stops the calling script.

- [ ] **Step 2: Write `scripts/git/branch-init.sh`**

```bash
#!/usr/bin/env bash
# Creates a branch from HEAD whose first commit is the empty marker `>>> branch: <name>`, and pushes it.
#
# Usage:
#   ./scripts/git/branch-init.sh <branch-name>

set -euo pipefail

source "$(dirname "$0")/git-preconditions.sh"

function main {
    if test "$#" -ne 1 || test -z "$1"; then
        echo "usage: $0 <branch-name>" >&2
        exit "${USAGE_EXIT_STATUS}"
    fi

    local branch_name="$1"

    require_git_repository
    require_clean_working_tree

    local local_branch
    local_branch=$(local_branch_exists "${branch_name}")

    if test "${local_branch}" = "present"; then
        echo "error: the local branch '${branch_name}' already exists" >&2
        exit 1
    fi

    local remote_branch
    remote_branch=$(remote_branch_exists "${branch_name}")

    if test "${remote_branch}" = "present"; then
        echo "error: the remote branch 'origin/${branch_name}' already exists" >&2
        exit 1
    fi

    git checkout -b "${branch_name}"
    git commit --allow-empty -m ">>> branch: ${branch_name}"
    git push -u origin "${branch_name}"
}

main "$@"
```

Run: `chmod +x ./scripts/git/branch-init.sh`

- [ ] **Step 3: Verify against a throwaway origin** (never against this repository's origin)

Write `/tmp/verify-branch-init.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

repository_root="$1"
sandbox=$(mktemp -d /tmp/verify-branch-init.XXXXXX)
echo "sandbox: ${sandbox}"

git init --quiet --bare --initial-branch=master "${sandbox}/origin.git"
git clone --quiet "${sandbox}/origin.git" "${sandbox}/clone" 2> /dev/null
cd "${sandbox}/clone"
git config user.email "verify@example.invalid"
git config user.name "verify"
mkdir -p ./scripts
cp -R "${repository_root}/scripts/git" ./scripts/git
git add ./scripts/git
git commit --quiet -m "initial"
git push --quiet -u origin master

./scripts/git/branch-init.sh phase-one > /dev/null 2>&1
echo "--- phase-one log"
git log --format=%s phase-one
echo "--- remote branches"
git ls-remote --heads origin | awk '{print $2}'

set +e
./scripts/git/branch-init.sh > /dev/null 2>&1; echo "without a name: exit $?"
./scripts/git/branch-init.sh phase-one > /dev/null 2>&1; echo "existing name: exit $?"
echo "dirty" >> ./scripts/git/branch-init.sh
./scripts/git/branch-init.sh phase-two > /dev/null 2>&1; echo "dirty working tree: exit $?"
```

Run: `chmod +x /tmp/verify-branch-init.sh && /tmp/verify-branch-init.sh /Users/singularity/bacter-rs`
Expected (the sandbox path varies):

```
sandbox: /tmp/verify-branch-init.XXXXXX
--- phase-one log
>>> branch: phase-one
initial
--- remote branches
refs/heads/master
refs/heads/phase-one
without a name: exit 64
existing name: exit 1
dirty working tree: exit 1
```

- [ ] **Step 4: Commit**

```sh
git status
git add ./scripts/git/git-preconditions.sh ./scripts/git/branch-init.sh
git commit -m "branch-init script"
```

### Task 7: cleanup-merged and pr-integrate scripts

**Files:**
- Create: `scripts/git/cleanup-merged.sh`
- Create: `scripts/git/pr-integrate.sh`

Adapted from eafora's scripts: the default branch comes from `origin/HEAD` instead of a literal `master`, statuses are captured before branching, and `pr-integrate.sh` has no `--budget` mode.

- [ ] **Step 1: Write `scripts/git/cleanup-merged.sh`**

```bash
#!/usr/bin/env bash
# Deletes merged branches from origin and locally, then prunes stale remote-tracking refs.
# Integration rebases, so a merged branch's tip is unreachable from the default branch and `git branch -d`
# would refuse; this deletes with -D.
#
# Usage:
#   ./scripts/git/cleanup-merged.sh <branch-name> [<branch-name>...]

set -euo pipefail

source "$(dirname "$0")/git-preconditions.sh"

function require_deletable_branches {
    local default_branch="$1"
    shift

    local current_branch
    current_branch=$(git rev-parse --abbrev-ref HEAD)

    for branch_name in "$@"; do
        if test "${branch_name}" = "${default_branch}"; then
            echo "error: refusing to delete the default branch '${default_branch}'" >&2
            exit 1
        fi

        if test "${branch_name}" = "${current_branch}"; then
            echo "error: cannot delete '${branch_name}' while it is checked out" >&2
            exit 1
        fi
    done
}

function delete_remote_branches {
    local remote_branch_names=()

    for branch_name in "$@"; do
        local remote_branch
        remote_branch=$(remote_branch_exists "${branch_name}")

        if test "${remote_branch}" = "present"; then
            remote_branch_names+=("${branch_name}")
        fi
    done

    if test "${#remote_branch_names[@]}" -eq 0; then
        return 0
    fi

    local push_status=0
    git push origin --delete "${remote_branch_names[@]}" 2> /dev/null || push_status=$?

    if test "${push_status}" -eq 0; then
        return 0
    fi

    # GitHub's automatic head-branch deletion can remove a branch between the listing above and the push.
    echo "note: the delete push failed; checking whether origin already deleted the branches"
    git fetch --prune origin > /dev/null

    for branch_name in "${remote_branch_names[@]}"; do
        local remote_branch
        remote_branch=$(remote_branch_exists "${branch_name}")

        if test "${remote_branch}" = "present"; then
            echo "error: the branch '${branch_name}' still exists on origin" >&2
            exit 1
        fi
    done
}

function delete_local_branches {
    local local_branch_names=()

    for branch_name in "$@"; do
        local local_branch
        local_branch=$(local_branch_exists "${branch_name}")

        if test "${local_branch}" = "present"; then
            local_branch_names+=("${branch_name}")
        fi
    done

    if test "${#local_branch_names[@]}" -gt 0; then
        git branch -D "${local_branch_names[@]}"
    fi
}

function main {
    if test "$#" -eq 0; then
        echo "usage: $0 <branch-name> [<branch-name>...]" >&2
        exit "${USAGE_EXIT_STATUS}"
    fi

    require_git_repository

    local default_branch
    default_branch=$(read_default_branch)

    require_deletable_branches "${default_branch}" "$@"
    delete_remote_branches "$@"
    delete_local_branches "$@"

    git remote prune origin
}

main "$@"
```

The empty-array guards matter: `/usr/bin/env bash` is bash 3.2 on macOS, where expanding an empty array under `set -u` is an unbound-variable error.

- [ ] **Step 2: Write `scripts/git/pr-integrate.sh`**

```bash
#!/usr/bin/env bash
# Integrates a pushed branch into the default branch by local rebase, keeping its `>>> branch: <name>` marker
# commit (GitHub's rebase button drops empty commits), then removes the branch.
#
# Usage:
#   ./scripts/git/pr-integrate.sh <branch>
#   ./scripts/git/pr-integrate.sh --current
#   ./scripts/git/pr-integrate.sh <branch> --from <former-parent-branch>
#
# --current integrates the checked-out branch. --from rebases a stacked branch whose parent already merged:
# only the commits after <former-parent-branch> move onto the default branch.

set -euo pipefail

source "$(dirname "$0")/git-preconditions.sh"

function print_usage {
    echo "usage: $0 (<branch> | --current) [--from <former-parent-branch>]" >&2
}

function print_help {
    awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"
}

function read_current_branch {
    local current_branch
    current_branch=$(git symbolic-ref --short HEAD 2> /dev/null || true)

    if test -z "${current_branch}"; then
        echo "error: --current needs HEAD on a branch, not detached" >&2
        exit 1
    fi

    echo "${current_branch}"
}

function require_integrable_branch {
    local branch_name="$1"
    local default_branch="$2"

    if test "${branch_name}" = "${default_branch}"; then
        echo "error: refusing to integrate the default branch '${default_branch}' into itself" >&2
        exit 1
    fi

    local local_branch
    local_branch=$(local_branch_exists "${branch_name}")

    if test "${local_branch}" = "absent"; then
        echo "error: the local branch '${branch_name}' does not exist" >&2
        exit 1
    fi

    local remote_branch
    remote_branch=$(remote_branch_exists "${branch_name}")

    if test "${remote_branch}" = "absent"; then
        echo "error: the remote branch 'origin/${branch_name}' does not exist; push it first" >&2
        exit 1
    fi
}

function require_resolvable_ref {
    local ref_name="$1"

    local resolved_commit
    resolved_commit=$(git rev-parse --verify --quiet "${ref_name}^{commit}" || true)

    if test -z "${resolved_commit}"; then
        echo "error: the former-parent ref '${ref_name}' does not resolve to a commit" >&2
        exit 1
    fi
}

function integrate {
    local branch_name="$1"
    local default_branch="$2"
    local former_parent_branch="$3"

    echo ">>> Updating ${default_branch}"
    git checkout "${default_branch}"
    git pull --ff-only

    echo ">>> Rebasing '${branch_name}' onto ${default_branch}"
    git checkout "${branch_name}"

    if test -n "${former_parent_branch}"; then
        git rebase --onto "${default_branch}" "${former_parent_branch}" "${branch_name}"
    else
        git rebase "${default_branch}"
    fi

    echo ">>> Force-pushing the rebased '${branch_name}'"
    git push --force-with-lease

    echo ">>> Fast-forwarding ${default_branch} to '${branch_name}'"
    git checkout "${default_branch}"
    git rebase "${branch_name}"

    echo ">>> Pushing ${default_branch}"
    git push origin "${default_branch}"

    echo ">>> Removing '${branch_name}'"
    "$(dirname "$0")/cleanup-merged.sh" "${branch_name}"

    echo ">>> Done"
}

function main {
    local branch_name=""
    local former_parent_branch=""
    local branch_source="argument"

    while test "$#" -gt 0; do
        case "$1" in
            --from)
                if test "$#" -lt 2; then
                    echo "error: --from needs a former-parent branch" >&2
                    exit "${USAGE_EXIT_STATUS}"
                fi

                former_parent_branch="$2"
                shift 2
                ;;
            --current)
                branch_source="current"
                shift
                ;;
            -h|--help)
                print_help
                exit 0
                ;;
            -*)
                echo "error: unknown flag: $1" >&2
                exit "${USAGE_EXIT_STATUS}"
                ;;
            *)
                if test -n "${branch_name}"; then
                    echo "error: unexpected argument: $1" >&2
                    exit "${USAGE_EXIT_STATUS}"
                fi

                branch_name="$1"
                shift
                ;;
        esac
    done

    if test "${branch_source}" = "current" && test -n "${branch_name}"; then
        echo "error: --current cannot be combined with a branch argument" >&2
        exit "${USAGE_EXIT_STATUS}"
    fi

    require_git_repository

    if test "${branch_source}" = "current"; then
        branch_name=$(read_current_branch)
    fi

    if test -z "${branch_name}"; then
        print_usage
        exit "${USAGE_EXIT_STATUS}"
    fi

    local default_branch
    default_branch=$(read_default_branch)

    require_clean_working_tree
    require_integrable_branch "${branch_name}" "${default_branch}"

    if test -n "${former_parent_branch}"; then
        require_resolvable_ref "${former_parent_branch}"
    fi

    integrate "${branch_name}" "${default_branch}" "${former_parent_branch}"
}

main "$@"
```

Run: `chmod +x ./scripts/git/cleanup-merged.sh ./scripts/git/pr-integrate.sh`

- [ ] **Step 3: Verify against a throwaway origin** (never against this repository's origin)

Write `/tmp/verify-git-scripts.sh`. It stacks `phase-two` on `phase-one`, advances `master` so the rebase rewrites `phase-one`, integrates both (the second with `--from` the former tip of `phase-one`), and checks the refusals.

```bash
#!/usr/bin/env bash
set -euo pipefail

repository_root="$1"
sandbox=$(mktemp -d /tmp/verify-git-scripts.XXXXXX)
echo "sandbox: ${sandbox}"

git init --quiet --bare --initial-branch=master "${sandbox}/origin.git"
git clone --quiet "${sandbox}/origin.git" "${sandbox}/clone" 2> /dev/null
cd "${sandbox}/clone"
git config user.email "verify@example.invalid"
git config user.name "verify"
mkdir -p ./scripts
cp -R "${repository_root}/scripts/git" ./scripts/git
git add ./scripts/git
git commit --quiet -m "initial"
git push --quiet -u origin master
git remote set-head origin --auto > /dev/null

./scripts/git/branch-init.sh phase-one > /dev/null 2>&1
echo "one" > ./one.txt
git add ./one.txt
git commit --quiet -m "phase one work"
git push --quiet

./scripts/git/branch-init.sh phase-two > /dev/null 2>&1
echo "two" > ./two.txt
git add ./two.txt
git commit --quiet -m "phase two work"
git push --quiet

git checkout --quiet master
echo "unrelated" > ./unrelated.txt
git add ./unrelated.txt
git commit --quiet -m "unrelated master work"
git push --quiet

phase_one_tip_before_integration=$(git rev-parse phase-one)
./scripts/git/pr-integrate.sh phase-one > /dev/null 2>&1
./scripts/git/pr-integrate.sh phase-two --from "${phase_one_tip_before_integration}" > /dev/null 2>&1

echo "--- master log"
git log --format=%s master
echo "--- local branches"
git branch --format='%(refname:short)'
echo "--- remote branches"
git ls-remote --heads origin | awk '{print $2}'

set +e
./scripts/git/cleanup-merged.sh master > /dev/null 2>&1; echo "cleanup-merged master: exit $?"
./scripts/git/pr-integrate.sh --current > /dev/null 2>&1; echo "pr-integrate --current on master: exit $?"
./scripts/git/pr-integrate.sh --bogus > /dev/null 2>&1; echo "pr-integrate --bogus: exit $?"
git remote set-head origin --delete
./scripts/git/cleanup-merged.sh some-branch 2>&1; echo "cleanup-merged without origin/HEAD: exit $?"
```

Run: `chmod +x /tmp/verify-git-scripts.sh && /tmp/verify-git-scripts.sh /Users/singularity/bacter-rs`
Expected (the sandbox path varies):

```
sandbox: /tmp/verify-git-scripts.XXXXXX
--- master log
phase two work
>>> branch: phase-two
phase one work
>>> branch: phase-one
unrelated master work
initial
--- local branches
master
--- remote branches
refs/heads/master
cleanup-merged master: exit 1
pr-integrate --current on master: exit 1
pr-integrate --bogus: exit 64
error: origin/HEAD is not set; run: git remote set-head origin --auto
cleanup-merged without origin/HEAD: exit 1
```

Run: `./scripts/git/pr-integrate.sh --help`
Expected: the header comment from `Integrates a pushed branch ...` through `... move onto the default branch.`, without `#` prefixes.

- [ ] **Step 4: Commit**

```sh
git status
git add ./scripts/git/cleanup-merged.sh ./scripts/git/pr-integrate.sh
git commit -m "pr-integrate and cleanup-merged scripts"
```

### Task 8: Coding conventions

**Files:**
- Create: `docs/conventions/logging.md`, `docs/conventions/conditional-compilation.md`, `docs/conventions/shading.md` (copied as is)
- Create: `docs/conventions/types.md` (adapted)
- Create: `docs/conventions/git-workflow.md`, `docs/conventions/testing.md` (new)
- Create: `docs/conventions/README.md` (adapted index)

- [ ] **Step 1: Copy the three unchanged docs**

Run: `mkdir -p ./docs/conventions && cp /Users/singularity/eafora/docs/conventions/logging.md /Users/singularity/eafora/docs/conventions/conditional-compilation.md /Users/singularity/eafora/docs/conventions/shading.md ./docs/conventions/`

- [ ] **Step 2: Write `docs/conventions/types.md`**

````markdown
# Type naming convention

Source of truth for how Rust types are named and laid out across the bacter-rs codebase. Memory files reference this document; they don't restate it.

The type-naming rules below are strict: the same kind of shape always gets the same suffix (`SerialOut`, `SerialIn`, `Serial`, `Kind`). Drift here makes `shared/src/protocol/` unreadable.

The variable-naming guidance at the end is softer: a default to fall back to, not a rule to enforce in code review.

## Underlying heuristic: hierarchical naming

The rules below all share one underlying preference: hierarchical naming over English-sounding naming. Most-significant noun first; modifiers and qualifiers after. Concretely for types:

- The model name comes first; the wire-shape suffix follows: `MemberSerialOut`, not `SerialOutMember`.
- Direction comes before redaction context: `MemberSerialOutPublic`, not `PublicMemberSerialOut`. Every same-direction variant shares the `MemberSerialOut...` prefix.
- Enum disambiguators trail the noun: `GameModeKind`, not `KindOfGameMode`.

Why: items sharing a hierarchical axis sort and grep together. `grep "MemberSerialOut"` finds every outbound variant of `Member`; an alphabetical listing puts every model's wire types adjacent. English-sounding orderings scatter related types across the namespace.

## Core dichotomy: model and wire type

Every type that crosses the WebSocket or is written to a replay file has two shapes:

- Model: what application code uses (simulation, server, client session). Bare-named (`GameState`, `Member`, `Loadout`). Never derives bitcode.
- Wire type: what bitcode encodes. Suffixed by direction (below). Derives `bitcode::Encode` and `bitcode::Decode`; wire types are the only types that do.

The two shapes are always two distinct structs, even when their fields are identical. The wire types mark the shapes that cannot change casually: changing a wire type changes the bytes on the wire and in replay files, so it bumps `PROTOCOL_VERSION`.

Wire types live in `shared/src/protocol/`, not in a `<feature>_model.rs`. No module outside `shared/src/protocol/`, and no other crate, names `bitcode`.

## Wire formats

Direction is the primary axis. Redaction is a secondary modifier and composes onto direction:

- `<Model>SerialOut`: server to client. Replay files are server output, so their records use `SerialOut` types, the header included (`ReplayHeaderSerialOut`).
- `<Model>SerialIn`: client to server. Often differs from `Out` because the client doesn't supply server-assigned fields.
- `<Model>Serial`: only when In and Out are byte-identical. Don't reach for this just to save a struct; if there's any chance In and Out will diverge later, write them separately from the start.
- `<Model>SerialOutPublic` and similar: context modifiers for redaction, suffixed after the direction. Don't introduce these speculatively; wait until there's a real second consumer.
- Wire-only types without a model (message envelopes such as `MessageSerialIn` and `MessageSerialOut`) have no conversions.
- Newtypes and bit sets are primitives on the wire.

## Conversion impl placement

- In the wire type's file in `shared/src/protocol/`, immediately after the wire type.
- Sending side: `impl From<&Model> for <Model>SerialOut` (or `<Model>Serial`), always infallible.
- Client input: `impl TryFrom<<Model>SerialIn> for Model` (client input can fail validation).
- Receiving side of server output (client decoding, replay reading): `impl TryFrom<<Model>SerialOut> for Model` or `impl TryFrom<<Model>Serial> for Model`. `From` only for plain enum mirrors, where nothing can fail.
- Receiving: `bitcode::decode` into the wire type, then `TryFrom` into the model. Sending: `From` into the wire type, then `bitcode::encode`.
- A model whose invariants sit in private fields owns them in its own module: raw-part accessors for the `From`, and a validating constructor returning `Option` which the `TryFrom` in `protocol/` calls.

## Enums

Two flavors:

- `<Noun>Kind`: when the bare name would shadow or be confused for a related struct. Without the suffix, a reader seeing `GameMode` reasonably assumes a data-bearing struct; the `Kind` suffix is the signal that this is an enumeration of variant tags. Use `Kind` whenever a related struct of the bare name exists, or when the bare name would otherwise read as a noun describing a thing rather than a classification.
- Bare descriptive name: when the type name already unambiguously reads as an enumeration of values: `RoundPhase`, `AbilityPhase`. The `Phase` suffix is doing the work `Kind` would.

Method naming for the string direction:

- `<field>()`: when the enum maps cleanly to a single named code whose name reads naturally as a method: `GameModeKind::code()` (`"ffa"`, `"skm"`, `"srv"`).
- `as_str()`: fallback for everything else.

Always implement `TryFrom<&str>` for the string to enum direction. This keeps the wire to model idiom uniform: `Model::try_from(wire)` works whether `wire` is a wire type or a `&str`. Don't implement `FromStr` instead: the `parse::<T>()` shortcut isn't load-bearing here, and having two near-equivalent traits just splits the codebase's idiom in half.

## Variable naming (guidance, not strict)

When in doubt, name variables after the type they hold, lowercase, plural for collections. The wire suffix is part of the variable name:

```rust
let member_serial_out: MemberSerialOut = MemberSerialOut::from(&member);
let member_serial_outs: Vec<MemberSerialOut> = members.iter().map(MemberSerialOut::from).collect();
```

Type naming is the load-bearing part of this spec; variable naming is preference. Don't burn review time on variable-rename churn.

## Database types

Deferred until a database exists: the `Entity` (table-row mirror) and `Projection` (join or subset) wire types, their conversions, and `query_scalar!` for single-column queries follow eafora's `docs/conventions/types.md` when persistence is added.

## Where bacter-rs diverges from Singularity

- Encoding: bitcode with its own `Encode`/`Decode` derive, in place of serde and rmp-serde. No serde until a consumer exists.
- Direction: Singularity uses `Serial` for outbound and `PublicSerial` for the redacted variant; bacter-rs uses the `SerialIn`/`SerialOut` split above, which is more honest about why two structs exist when they do.
````

- [ ] **Step 3: Write `docs/conventions/git-workflow.md`**

```markdown
# Git workflow

Governance for branches, commits, pull requests and integration. Each rule names the script which implements it, where one exists.

## Default branch

- `master`, locally and on GitHub.
- Scripts read it from `origin/HEAD` (`git symbolic-ref --short refs/remotes/origin/HEAD`). A repository whose origin was added with `git remote add` rather than cloned sets it once with `git remote set-head origin --auto`.

## Branches

- One short-lived branch per body of work, from the latest default branch.
- Serial phases of one plan are stacked: each phase branches from the previous phase's branch, and each pull request targets its parent branch.
- The first commit on every branch is the empty marker `>>> branch: <name>`, created by `./scripts/git/branch-init.sh <name>`. Integration rebases, so the markers are what keeps pull request boundaries visible in the default branch's history: `git log --grep '^>>> branch:'`.

## Commits

- One-line messages, no body, no attribution lines.
- Stage by explicit path; never `git add -A` or `git add .`; check `git status` before committing.
- Commit after each phase of a change; add a commit for a cleanup rather than amending.
- Every commit is pushed at once.

## Pull requests

- `gh pr create --assignee @me`, targeting the parent branch.
- The description is a release note: the problem and the solution in a few sentences, no test plan.

## Integration

- By local rebase through `./scripts/git/pr-integrate.sh <branch>` (or `--current`), never the GitHub rebase button, which drops the empty marker commits.
- A stacked branch whose parent already merged: `./scripts/git/pr-integrate.sh <branch> --from <former-parent>`, where `<former-parent>` is the parent branch or its former tip commit.
- Force pushes only with `--force-with-lease`.
- Merged branches are removed with `./scripts/git/cleanup-merged.sh <branch>...`; `pr-integrate.sh` runs it for the branch it integrates.

## Script registry

- `scripts/git/branch-init.sh`: a branch from HEAD with the marker commit, pushed with upstream tracking.
- `scripts/git/pr-integrate.sh`: integration by local rebase, fast-forward of the default branch, removal of the branch.
- `scripts/git/cleanup-merged.sh`: removal of merged branches from origin and locally, then `git remote prune origin`.
- `scripts/git/git-preconditions.sh`: the precondition checks the scripts above source; not run directly.
```

- [ ] **Step 4: Write `docs/conventions/testing.md`**

```markdown
# Testing conventions

What is written test-first, what is exempt, and the defaults every test follows. The tests themselves are listed in `docs/architecture/testing.md`.

## Test-first

Written test-first (a failing test, then the implementation which makes it pass):

- simulation rules;
- determinism: checksums, replay fixtures, forbidden operations, chance tables, `Pcg32`;
- the protocol codec and validation, including every `TryFrom` rejection;
- server admission and cursor clamp logic.

## Exempt

- UI views (Leptos components) and shaders. Views are verified manually; shaders by the ignored pipeline-compile test.

## Defaults

Singularity parity unless a divergence is recorded below.

- Unit tests sit inline at the bottom of the file under test, in `#[cfg(test)] mod tests { use super::*; ... }`.
- Integration tests sit in `<crate>/tests/`. A helper used by two or more test files lives in `tests/helpers/<concern>.rs`; a helper with one consumer stays a private function in that test file.
- Test names: `<function_under_test>_<scenario>`, for example `from_parts_rejects_even_increment`.
- No messages in assertions.
- A test which needs a GPU, a browser, wasmtime or a long run is `#[ignore]` and runs through a script in `scripts/test/`.
- `#[wasm_bindgen_test]` only for behaviour which exists only in a browser. Target-agnostic logic is covered by the host `#[test]` plus `./scripts/test/check-wasm.sh`.
- A simulation change or a wire-type change keeps the replay fixtures passing, or regenerates the fixtures and their checksums deliberately.

## Where bacter-rs diverges from Singularity

- The inline unit-test module is named `tests`, not `test`.
```

- [ ] **Step 5: Write `docs/conventions/README.md`**

```markdown
# Coding conventions

Per-topic convention specs that govern the bacter-rs codebase. Read the relevant doc before writing new code in that area.

These docs are the source of truth. Plans and memory files (in `~/.claude/projects/-Users-singularity-bacter-rs/memory/`) reference them rather than restating rules.

## Index

- [`types.md`](types.md): type naming (models and their `SerialIn`/`SerialOut`/`Serial` wire types, conversion placement, `Kind` enum suffix, `code()`/`as_str()`, `TryFrom<&str>` parsing).
- [`logging.md`](logging.md): log message format: `<message>; [key=value ...]` for messages with structured data; prose-only messages stand alone.
- [`shading.md`](shading.md): WGSL and shader naming: transform matrices named `<source_space>_to_<destination_space>` (`object_to_world`, `view_to_clip`), never by pipeline role alone (`model_matrix`) or acronym (`mvp`).
- [`conditional-compilation.md`](conditional-compilation.md): gate a block of target- or feature-specific items in one `#[cfg]`-ed submodule (`mod native`/`mod wasm`, `mod hydrate`/`mod ssr`) with the `#[cfg]` on the module and re-exports, not per item; one-line WHY on each gate.
- [`git-workflow.md`](git-workflow.md): default branch, stacked branches and marker commits, commits, pull requests, integration by local rebase, and the script implementing each rule.
- [`testing.md`](testing.md): what is written test-first, what is exempt, and the test defaults.

## When to add a doc here

Add a convention doc when a rule:

1. Affects how multiple features get written (not a one-off architectural choice).
2. Has been called out in code review more than once.
3. Has tradeoffs worth recording so future-you doesn't re-derive them.

Don't add a doc just to centralize trivia that's already obvious from the code.
```

- [ ] **Step 6: Verify**

Run: `for name in logging conditional-compilation shading; do diff "/Users/singularity/eafora/docs/conventions/${name}.md" "./docs/conventions/${name}.md" && echo "${name}: identical"; done`
Expected:

```
logging: identical
conditional-compilation: identical
shading: identical
```

Run: `grep -n -i -E 'eafora|constitution|spec kit|\.specify|specs/' ./docs/conventions/README.md ./docs/conventions/types.md ./docs/conventions/git-workflow.md ./docs/conventions/testing.md`
Expected: exactly one match, the line of `types.md` under "Database types" containing `follow eafora's`.

Run: `ls ./docs/conventions`
Expected (macOS `ls` sorts case-insensitively):

```
conditional-compilation.md
git-workflow.md
logging.md
README.md
shading.md
testing.md
types.md
```

- [ ] **Step 7: Commit**

```sh
git status
git add ./docs/conventions/README.md ./docs/conventions/logging.md ./docs/conventions/conditional-compilation.md ./docs/conventions/shading.md ./docs/conventions/types.md ./docs/conventions/git-workflow.md ./docs/conventions/testing.md
git commit -m "coding conventions"
```

### Task 9: Project CLAUDE.md

**Files:**
- Create: `CLAUDE.md`

Contents per `docs/architecture/dev-workflow.md` (`CLAUDE.md` for bacter-rs); the poisoned-lock and input-handler naming rules are worded after `/Users/singularity/singularity/CLAUDE.md`.

- [ ] **Step 1: Write `CLAUDE.md`**

```markdown
# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Project

- Rust rewrite of Bacter.
- Reference implementation: `/Users/singularity/bacter`, branch `master`; read-only.
- Models: `/Users/singularity/eafora` (workspace shape, Leptos web, wgpu renderer in `shared`, `docs/conventions/`, `scripts/`) and `/Users/singularity/singularity` (actix-web and actix-ws server patterns).
- Everything runs on this machine. Never require a hosted server (production, staging, shared) to develop or verify a change.

## Documentation

- Architecture: `docs/architecture/`, indexed by `docs/architecture/README.md`. Binding; plans elaborate it and do not reopen its decisions.
- Conventions: `docs/conventions/`, indexed by `docs/conventions/README.md`, including `git-workflow.md` and `testing.md`. Read the relevant doc before writing code in that area.
- Plans: `docs/plans/`.

## Determinism contract

- The simulation in `shared` is bit-identical on wasm32 and native.
- Integer state only; no `f32`/`f64` stored in `GameState`.
- Floats only in pure functions with integer inputs and outputs, using only IEEE-exact operations; no `mul_add`.
- Logarithm only through `libm::log`, only while building the growth chance tables.
- Randomness only from the hand-written `Pcg32`, in the defined draw order.
- No `HashMap` or `HashSet`; no `usize` in `GameState` or any wire type; iteration in the defined orders.
- Enforced by `shared/tests/forbidden_operations.rs`.
- Every simulation change or wire-type change keeps the replay fixtures passing, or regenerates the fixtures and their checksums deliberately.

## Wire types

- Only the `...Serial*` types in `shared/src/protocol/` derive bitcode; no other module or crate names bitcode.
- Changing a wire type bumps `PROTOCOL_VERSION`.

## Scripts

- Invoke the wrappers in `scripts/`, never the raw tool they wrap, including in commands written for the owner. Check for a wrapper before writing a raw tool command.

## Code rules

- `mod.rs` holds only `pub mod x;` + `pub use x::*;` pairs and doc comments.
- File order: imports, consts and statics, types each followed by its `impl`, free functions.
- `RwLock` and `Mutex` poison errors are unwrapped (`.unwrap()` or `.expect()`), never handled: a poisoned lock means a thread panicked.
- Input handler naming: trait functions (`ClickHandler::click`) use handle semantics, where the implementor checks whether the event applies; callback fields and delegating functions (`on_click`) use on semantics, where the caller has already established that the event is relevant.
- New dependencies are discussed with the owner first.

## Owner preferences

- `~/.claude/CLAUDE.md` holds the owner's preferences for code style, commits, tool use and process. Read it.
```

- [ ] **Step 2: Verify**

Run: `grep -c '' ./CLAUDE.md && tail -c 1 ./CLAUDE.md | xxd -p`
Expected: a line count, then `0a` (the file ends with a newline).

Run: `grep -n -E 'docs/conventions/|docs/architecture/|PROTOCOL_VERSION|forbidden_operations' ./CLAUDE.md`
Expected: matches in the Documentation, Determinism contract and Wire types sections.

- [ ] **Step 3: Commit**

```sh
git status
git add ./CLAUDE.md
git commit -m "project CLAUDE.md"
```

### Task 10: Phase completion check

**Files:** none (verification only; no commit).

- [ ] **Step 1: Run the roadmap's done criteria from a clean build**

Run: `cargo clean && cargo build --workspace && cargo test --workspace && ./scripts/test/check-wasm.sh; echo "exit=$?"`
Expected: the workspace builds without warnings, every `test result:` line is `ok`, the three wasm32 checks end with `Finished`, then `exit=0`.

- [ ] **Step 2: Confirm the tree**

Run: `git status --short`
Expected: no output.

Run: `git log --oneline 0124559..HEAD`
Expected, newest first (hashes vary):

```
<hash> project CLAUDE.md
<hash> coding conventions
<hash> pr-integrate and cleanup-merged scripts
<hash> branch-init script
<hash> dev server script
<hash> setup script and environment template
<hash> wasm32 check script
<hash> workspace and empty crate shells
<hash> toolchain and rustfmt configuration
<hash> phase 1 plan
```

Run: `git ls-files | grep -v -E '^docs/(architecture|plans)/'`
Expected:

```
.gitignore
CLAUDE.md
Cargo.lock
Cargo.toml
docs/conventions/README.md
docs/conventions/conditional-compilation.md
docs/conventions/git-workflow.md
docs/conventions/logging.md
docs/conventions/shading.md
docs/conventions/testing.md
docs/conventions/types.md
rust-toolchain.toml
rustfmt.toml
scripts/dev/server.sh
scripts/git/branch-init.sh
scripts/git/cleanup-merged.sh
scripts/git/git-preconditions.sh
scripts/git/pr-integrate.sh
scripts/test/check-wasm.sh
server/Cargo.toml
server/src/lib.rs
server/src/main.rs
setup.sh
shared/Cargo.toml
shared/src/lib.rs
template.env
tools/protocol_dump/Cargo.toml
tools/protocol_dump/src/main.rs
web/Cargo.toml
web/src/lib.rs
web/src/main.rs
```

## Roadmap coverage

- Root `Cargo.toml` (members, `[workspace.dependencies]`, `wasm-release` profile): Task 2.
- `rust-toolchain.toml`, `rustfmt.toml`: Task 1.
- `template.env`, `setup.sh`: Task 4.
- `CLAUDE.md`: Task 9.
- Crates `shared`, `server`, `web`, `tools/protocol_dump` as empty shells with final feature flags and per-target tables: Task 2.
- `docs/conventions/` (README, logging, conditional-compilation, shading, types, git-workflow, testing): Task 8.
- `scripts/` skeleton with the scripts that already have something to run: Tasks 3, 5, 6, 7.
- Done criteria (`cargo build --workspace`, `cargo test --workspace`, `./scripts/test/check-wasm.sh`): Task 10.
