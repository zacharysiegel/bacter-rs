#!/usr/bin/env bash
# Builds the deployable site into target/site: a release build, the prerendered document, and a
# game-server.json naming the page's own origin.
#
# Usage:
#   ./scripts/build/build-site.sh

set -euo pipefail

readonly MISSING_PROGRAM_EXIT_STATUS=1

function fail {
    echo "error: $1" >&2
    exit 1
}

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

# `cargo leptos watch` serves this same site root, and a release build replaces everything in it.
function refuse_while_the_dev_server_runs {
    local web_manifest_path="$1"

    local site_address
    site_address=$(grep -E '^site-addr[[:space:]]*=' "${web_manifest_path}" | head -1 | sed -E 's/^[^=]*=[[:space:]]*"(.*)".*$/\1/')

    local site_port="${site_address##*:}"

    local listener_ids
    listener_ids=$(lsof -nP -t -iTCP:"${site_port}" -sTCP:LISTEN || true)

    if test -n "${listener_ids}"; then
        fail "something listens on ${site_address}, where the dev server serves the site root this build replaces; stop it first"
    fi
}

function main {
    local repository_root
    repository_root=$(git rev-parse --show-toplevel)

    local web_directory="${repository_root}/web"
    local site_directory="${repository_root}/target/site"
    local web_binary="${repository_root}/target/release/web"

    require_program "cargo-leptos" "cargo install --locked cargo-leptos"
    require_program "jq" "brew install jq"
    require_program "lsof" "the lsof package of the system's package manager"
    refuse_while_the_dev_server_runs "${web_directory}/Cargo.toml"

    # Hashed names let the server cache /pkg/* as immutable; both the build and the prerender read this.
    export LEPTOS_HASH_FILES=true

    echo "building the release site"
    cd "${web_directory}"
    cargo leptos build --release

    if test ! -x "${web_binary}"; then
        fail "the release build produced no web binary at ${web_binary}"
    fi

    echo "prerendering the document"
    "${web_binary}" prerender

    # A null URL makes the page connect to its own origin.
    echo "naming the page's origin in game-server.json"
    jq '.websocket_url = null' "${web_directory}/static/game-server.json" > "${site_directory}/game-server.json"

    "${repository_root}/scripts/build/verify-site-tree.sh" "${site_directory}"
}

main
