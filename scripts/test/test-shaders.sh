#!/usr/bin/env bash
# Runs the ignored test which builds every render pipeline on a headless GPU device; WGSL is validated only then.

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

cargo test -p shared --features render --lib render:: -- --ignored
