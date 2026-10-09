#!/usr/bin/env bash
# Runs the ignored growth statistics test, which compares growth against means measured from the original rules.

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}"

cargo test -p shared --release --test growth_statistics -- --ignored --nocapture
