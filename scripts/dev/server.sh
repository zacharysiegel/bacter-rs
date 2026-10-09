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
