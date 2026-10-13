# System dependencies

Programs a development machine needs. `./setup.sh` checks for them, adds the Rust targets and creates `.env`; it installs nothing else.

## Required

- Rust toolchain through `rustup` (https://rustup.rs); `rust-toolchain.toml` pins the channel.
- Rust targets `wasm32-unknown-unknown` (web client, `./scripts/test/check-wasm.sh`) and `wasm32-wasip1` (replay test under wasmtime); `./setup.sh` adds both.
- `cargo-leptos`: `cargo install --locked cargo-leptos`; builds and serves the web client.
- `jq`: `brew install jq`; used by the site build.
- `wasmtime`: `brew install wasmtime`; runs the wasm32-wasip1 replay test (`./scripts/test/test-replay-wasm.sh`).

## Headless browser tests

Needed only by `./scripts/test/test-wasm.sh`.

- `wasm-pack`: `cargo install wasm-pack`.
- Google Chrome.
- `chromedriver` on `PATH`, the same major version as the installed Chrome, from Chrome for Testing (https://googlechromelabs.github.io/chrome-for-testing/). Chrome updates itself, so update chromedriver when the test reports a version mismatch.
