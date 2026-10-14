#!/usr/bin/env bash
# Setup of a development or production machine: installs the missing required programs and creates .env; safe to re-run.

set -euo pipefail

readonly MISSING_PROGRAM_EXIT_STATUS=1
readonly HOMEBREW_INSTALL_HINT='/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)" (https://brew.sh)'
readonly RUSTUP_INSTALLER_URL="https://sh.rustup.rs"
readonly RUST_TARGETS=(
    wasm32-unknown-unknown
    wasm32-wasip1
)

function fail_missing_program {
    echo "error: $1" >&2
    exit "${MISSING_PROGRAM_EXIT_STATUS}"
}

function require_homebrew {
    add_homebrew_programs_to_path

    local brew_path
    brew_path=$(command -v brew || true)

    if test -z "${brew_path}"; then
        fail_missing_program "Homebrew is required and setup.sh does not install it; install it with: ${HOMEBREW_INSTALL_HINT}"
    fi

    echo "keeping Homebrew at ${brew_path}"
}

function install_program_when_missing {
    local program_name="$1"
    shift
    local install_command=("$@")

    local program_path
    program_path=$(command -v "${program_name}" || true)

    if test -n "${program_path}"; then
        echo "keeping \`${program_name}\` at ${program_path}"
        return 0
    fi

    echo "installing \`${program_name}\` with: ${install_command[*]}"
    "${install_command[@]}"

    require_installed_program "${program_name}" "${install_command[*]}"
}

function install_rustup_when_missing {
    local rustup_path
    rustup_path=$(command -v rustup || true)

    if test -n "${rustup_path}"; then
        echo "keeping \`rustup\` at ${rustup_path}"
        return 0
    fi

    local install_command_text="curl --proto '=https' --tlsv1.2 -sSf ${RUSTUP_INSTALLER_URL} | sh -s -- -y --default-toolchain none"
    echo "installing \`rustup\` with: ${install_command_text}"
    curl --proto '=https' --tlsv1.2 -sSf "${RUSTUP_INSTALLER_URL}" | sh -s -- -y --default-toolchain none

    add_cargo_programs_to_path

    require_installed_program "rustup" "${install_command_text}"
}

function require_installed_program {
    local program_name="$1"
    local install_command_text="$2"

    local installed_program_path
    installed_program_path=$(command -v "${program_name}" || true)

    if test -z "${installed_program_path}"; then
        fail_missing_program "\`${program_name}\` is not on PATH after: ${install_command_text}"
    fi

    echo "installed \`${program_name}\` at ${installed_program_path}"
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

    source ./scripts/build/program-paths.sh

    require_homebrew

    add_cargo_programs_to_path
    install_rustup_when_missing

    # Installs the channel pinned in ./rust-toolchain.toml; needs rustup 1.28 or later.
    rustup toolchain install --no-update
    rustup target add "${RUST_TARGETS[@]}"

    install_program_when_missing "jq" brew install jq
    install_program_when_missing "wasmtime" brew install wasmtime
    install_program_when_missing "cargo-leptos" cargo install --locked cargo-leptos

    recommend_program "wasm-pack" "the headless browser tests" "cargo install wasm-pack"
    recommend_program "chromedriver" "the headless browser tests" "a build matching the installed Chrome major version, on PATH"

    create_environment_file

    echo "setup complete"
}

main
