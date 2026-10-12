#!/usr/bin/env bash
# Runs the replay fixture test built for wasm32-wasip1 under wasmtime, so a 32-bit usize and wasm float behaviour
# must reproduce the committed checksums.
#
# Extra arguments are passed to the test binary, for example:
#   ./scripts/test/test-replay-wasm.sh every_replay_fixture

set -euo pipefail

source "$(dirname "$0")/test-preconditions.sh"

readonly WASI_TARGET="wasm32-wasip1"
readonly FIXTURE_DIRECTORY="shared/tests/fixtures"

function require_rust_target {
    local target_name="$1"

    local installed_target
    installed_target=$(rustup target list --installed | grep -x "${target_name}" || true)

    if test -z "${installed_target}"; then
        echo "error: the \`${target_name}\` Rust target is required; install it with: rustup target add ${target_name}" >&2
        exit "${MISSING_PROGRAM_EXIT_STATUS}"
    fi
}

require_program "wasmtime" "brew install wasmtime"
require_rust_target "${WASI_TARGET}"

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

# The test reads its fixtures by their absolute host path, so the guest sees that directory at the same path.
export CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime run --dir=${repository_root}/${FIXTURE_DIRECTORY}"

# A panic aborts the wasm process before libtest prints the output it captured.
cargo test -p shared --test replay --target "${WASI_TARGET}" -- --nocapture "$@"
