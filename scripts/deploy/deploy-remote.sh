#!/usr/bin/env bash
# Runs ./scripts/deploy/deploy.sh on the production host over one ssh connection.
#
# Usage:
#   ./scripts/deploy/deploy-remote.sh [--bootstrap] [<ssh host>]
#
# --bootstrap clones the repository first when the repository path does not exist on the host.
# BACTER_REMOTE_REPOSITORY overrides the repository path on the host; the host's shell expands it.

set -euo pipefail

readonly DEFAULT_SSH_HOST="zachary@ssh.zach.ro"
readonly DEFAULT_REMOTE_REPOSITORY='~/bacter-rs'
readonly REPOSITORY_URL="https://github.com/zacharysiegel/bacter-rs.git"
readonly USAGE_EXIT_STATUS=64

function fail_usage {
    echo "error: $1" >&2
    echo "usage: ./scripts/deploy/deploy-remote.sh [--bootstrap] [<ssh host>]" >&2
    exit "${USAGE_EXIT_STATUS}"
}

function quote_for_remote_shell {
    local text="$1"
    local single_quote="'"
    local escaped_single_quote="'\\''"

    printf "'%s'" "${text//${single_quote}/${escaped_single_quote}}"
}

function create_remote_script {
    local repository_path="$1"
    local bootstrap_state="$2"

    local deploy_command="cd ${repository_path} && ./scripts/deploy/deploy.sh"

    if test "${bootstrap_state}" = "requested"; then
        echo "if test ! -d ${repository_path}; then git clone ${REPOSITORY_URL} ${repository_path}; fi && ${deploy_command}"
    else
        echo "${deploy_command}"
    fi
}

function main {
    local bootstrap_state="not requested"
    local ssh_host=""

    while test "$#" -gt 0; do
        case "$1" in
            --bootstrap) bootstrap_state="requested" ;;
            -*) fail_usage "unknown option $1" ;;
            *)
                if test -n "${ssh_host}"; then
                    fail_usage "more than one host given"
                fi

                ssh_host="$1"
                ;;
        esac

        shift
    done

    ssh_host="${ssh_host:-${DEFAULT_SSH_HOST}}"
    local repository_path="${BACTER_REMOTE_REPOSITORY:-${DEFAULT_REMOTE_REPOSITORY}}"

    local remote_script
    remote_script=$(create_remote_script "${repository_path}" "${bootstrap_state}")

    local quoted_remote_script
    quoted_remote_script=$(quote_for_remote_shell "${remote_script}")

    echo "running on ${ssh_host}: ${remote_script}"

    # A login shell, so the host's profile puts cargo and Homebrew programs on PATH.
    ssh "${ssh_host}" "exec \"\$SHELL\" -lc ${quoted_remote_script}"
}

main "$@"
