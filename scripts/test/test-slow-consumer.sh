#!/usr/bin/env bash
# Runs the ignored game server test of a member which stops reading.

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

cargo test -p server --test game_lifecycle -- --ignored
