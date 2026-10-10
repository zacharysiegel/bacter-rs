#!/usr/bin/env bash
# Rebuilds the scripted game fixture, then rewrites every fixture's golden checksums from the current simulation.

set -euo pipefail

readonly FIXTURE_DIRECTORY="shared/tests/fixtures"

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

cargo test -p shared --test replay -- --ignored --exact regenerate_scripted_game_fixture
cargo build -p protocol_dump

for replay_path in "${FIXTURE_DIRECTORY}"/*.replay; do
    checksums_path="${replay_path%.replay}.checksums"
    cargo run -q -p protocol_dump -- replay --checksums "${replay_path}" > "${checksums_path}"
    echo "wrote ${checksums_path}"
done
