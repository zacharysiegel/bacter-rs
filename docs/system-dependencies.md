# System dependencies

Programs a development machine needs. `./setup.sh` installs the required programs which are missing, except Homebrew, adds the Rust toolchain and targets, and creates `.env`; it only reports the headless browser test programs.

## Required

- Homebrew (https://brew.sh), on `PATH` or at `/opt/homebrew/bin/brew`; `./setup.sh` stops with the install command when it is absent.
- `rustup` (https://rustup.rs): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none`.
- The Rust toolchain pinned in `rust-toolchain.toml`: `rustup toolchain install --no-update` from the repository root (rustup 1.28 or later).
- Rust targets `wasm32-unknown-unknown` (web client, `./scripts/test/check-wasm.sh`) and `wasm32-wasip1` (replay test under wasmtime): `rustup target add wasm32-unknown-unknown wasm32-wasip1`.
- `cargo-leptos`: `cargo install --locked cargo-leptos`; builds and serves the web client.
- `jq`: `brew install jq`; used by the site build.
- `wasmtime`: `brew install wasmtime`; runs the wasm32-wasip1 replay test (`./scripts/test/test-replay-wasm.sh`).

## Headless browser tests

Needed only by `./scripts/test/test-wasm.sh`.

- `wasm-pack`: `cargo install wasm-pack`.
- Google Chrome.
- `chromedriver` on `PATH`, the same major version as the installed Chrome, from Chrome for Testing (https://googlechromelabs.github.io/chrome-for-testing/). Chrome updates itself, so update chromedriver when the test reports a version mismatch.
