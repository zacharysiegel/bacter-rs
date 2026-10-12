#!/usr/bin/env bash
# Builds the site and the release game server, which serves the site and the game socket from one origin.
#
# Usage:
#   ./scripts/build/build-release.sh

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

./scripts/build/build-site.sh
cargo build --release -p server
