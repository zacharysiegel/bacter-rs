#!/usr/bin/env bash
# Runs the web crate's #[wasm_bindgen_test] cases in headless Chrome.
# wasm-pack needs a package manifest, so this runs from web/.
#
# Extra arguments are passed to wasm-pack, for example:
#   ./scripts/test/test-wasm.sh -- load_returns

set -euo pipefail

readonly MISSING_PROGRAM_EXIT_STATUS=1

function require_program {
    local program_name="$1"
    local install_hint="$2"

    local program_path
    program_path=$(command -v "${program_name}" || true)

    if test -z "${program_path}"; then
        echo "error: the \`${program_name}\` program is required; install it with: ${install_hint}" >&2
        exit "${MISSING_PROGRAM_EXIT_STATUS}"
    fi
}

require_program "wasm-pack" "cargo install wasm-pack"
require_program "chromedriver" "a build matching the installed Chrome major version, on PATH"

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}/web"

exec wasm-pack test --headless --chrome --features hydrate "$@"
