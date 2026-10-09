#!/usr/bin/env bash
# Type-checks the wasm32 builds.

set -euo pipefail

readonly WASM_TARGET="wasm32-unknown-unknown"

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

cargo check -p shared --target "${WASM_TARGET}"
cargo check -p shared --target "${WASM_TARGET}" --features render
cargo check -p web --target "${WASM_TARGET}" --features hydrate
