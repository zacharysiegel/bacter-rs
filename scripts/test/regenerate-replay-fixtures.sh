#!/usr/bin/env bash
# Rewrites the golden checksums of the scripted game from the current simulation.

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

cargo test -p shared --test determinism -- --ignored --exact regenerate_scripted_game_checksums
