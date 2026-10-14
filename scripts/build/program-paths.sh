# Puts the Homebrew and rustup program directories on PATH for the sourcing script; sourced, not run.

readonly HOMEBREW_PROGRAM_DIRECTORY="/opt/homebrew/bin"
# Written by the rustup installer.
readonly CARGO_ENVIRONMENT_PATH="${HOME}/.cargo/env"

function add_homebrew_programs_to_path {
    local brew_path
    brew_path=$(command -v brew || true)

    if test -n "${brew_path}"; then
        return 0
    fi

    if test -x "${HOMEBREW_PROGRAM_DIRECTORY}/brew"; then
        PATH="${HOMEBREW_PROGRAM_DIRECTORY}:${PATH}"
    fi
}

function add_cargo_programs_to_path {
    if test -f "${CARGO_ENVIRONMENT_PATH}"; then
        source "${CARGO_ENVIRONMENT_PATH}"
    fi
}
