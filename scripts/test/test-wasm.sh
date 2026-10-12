#!/usr/bin/env bash
# Runs the web crate's #[wasm_bindgen_test] cases in headless Chrome.
# wasm-pack needs a package manifest, so this runs from web/.
#
# Extra arguments are passed to wasm-pack, for example:
#   ./scripts/test/test-wasm.sh -- load_returns

set -euo pipefail

source "$(dirname "$0")/test-preconditions.sh"

require_program "wasm-pack" "cargo install wasm-pack"
require_program "chromedriver" "a build matching the installed Chrome major version, on PATH"

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}/web"

exec wasm-pack test --headless --chrome --features hydrate "$@"
