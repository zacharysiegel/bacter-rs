#!/usr/bin/env bash
# One-time setup of a development machine; safe to re-run.

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

function recommend_program {
    local program_name="$1"
    local needed_by="$2"
    local install_hint="$3"

    local program_path
    program_path=$(command -v "${program_name}" || true)

    if test -z "${program_path}"; then
        echo "note: \`${program_name}\` is not installed and is needed by ${needed_by}; install it with: ${install_hint}"
    fi
}

function create_environment_file {
    if test -f ./.env; then
        echo "keeping the existing ./.env"
        return 0
    fi

    cp ./template.env ./.env
    echo "created ./.env from ./template.env"
}

function main {
    local repository_root
    repository_root=$(git rev-parse --show-toplevel)
    cd "${repository_root}"

    require_program "cargo" "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    require_program "cargo-leptos" "cargo install --locked cargo-leptos"
    require_program "jq" "brew install jq"

    rustup target add wasm32-unknown-unknown wasm32-wasip1

    recommend_program "wasm-pack" "the headless browser tests" "cargo install wasm-pack"
    recommend_program "chromedriver" "the headless browser tests" "a build matching the installed Chrome major version, on PATH"
    recommend_program "wasmtime" "the wasm32-wasip1 replay test" "brew install wasmtime"

    create_environment_file

    echo "setup complete"
}

main
