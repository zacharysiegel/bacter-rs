#!/usr/bin/env bash
# Runs the release game server with ./.env, after checking the settings a production server needs.
#
# Usage:
#   ./scripts/run/run-release-server.sh

set -euo pipefail

readonly REQUIRED_VARIABLES=(
    BACTER_BIND_ADDRESS
    BACTER_PORT
    BACTER_SITE_ROOT
    BACTER_ALLOWED_ORIGINS
)

function fail {
    echo "error: $1" >&2
    exit 1
}

function require_variables {
    for variable_name in "${REQUIRED_VARIABLES[@]}"; do
        local variable_value="${!variable_name:-}"

        if test -z "${variable_value}"; then
            fail "${variable_name} is not set in ./.env"
        fi
    done
}

function require_site_root {
    if test ! -f "${BACTER_SITE_ROOT}/index.html"; then
        fail "${BACTER_SITE_ROOT}/index.html is missing; run ./scripts/build/build-release.sh"
    fi
}

# TLS needs both PEM files, or neither for plain HTTP.
function require_tls_files {
    local certificate_path="${BACTER_TLS_CERTIFICATE_PATH:-}"
    local private_key_path="${BACTER_TLS_PRIVATE_KEY_PATH:-}"

    if test -z "${certificate_path}" && test -z "${private_key_path}"; then
        return 0
    fi

    if test -z "${certificate_path}" || test -z "${private_key_path}"; then
        fail "set both BACTER_TLS_CERTIFICATE_PATH and BACTER_TLS_PRIVATE_KEY_PATH, or neither"
    fi

    for pem_path in "${certificate_path}" "${private_key_path}"; do
        if test ! -r "${pem_path}"; then
            fail "${pem_path} is not readable"
        fi
    done
}

function main {
    local repository_root
    repository_root=$(git rev-parse --show-toplevel)
    cd "${repository_root}"

    if test ! -f ./.env; then
        fail "./.env is missing; run ./setup.sh first"
    fi

    unset "${REQUIRED_VARIABLES[@]}" BACTER_TLS_CERTIFICATE_PATH BACTER_TLS_PRIVATE_KEY_PATH

    set -a
    source ./.env
    set +a

    require_variables
    require_site_root
    require_tls_files

    if test ! -x ./target/release/server; then
        fail "./target/release/server is missing; run ./scripts/build/build-release.sh"
    fi

    exec ./target/release/server
}

main
