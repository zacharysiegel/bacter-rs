#!/usr/bin/env bash
# Deploys the latest master on this host: pulls, checks ./.env, builds, then restarts the server's LaunchAgent and waits for /health.
#
# Usage:
#   ./scripts/deploy/deploy.sh

set -euo pipefail

readonly DEPLOY_BRANCH="master"
readonly PRODUCTION_TEMPLATE_PATH="./scripts/deploy/production.template.env"
readonly PRODUCTION_VARIABLES=(
    BACTER_BIND_ADDRESS
    BACTER_PORT
    BACTER_SITE_ROOT
    BACTER_ALLOWED_ORIGINS
    BACTER_TRUSTED_PROXY_ADDRESSES
)
# Internal: marks the re-run of the pulled copy of this script.
readonly AFTER_PULL_OPTION="--after-pull"
readonly BOOTOUT_TIMEOUT_SECONDS=10
# EINPROGRESS; launchctl bootout returns it while the job is still exiting.
readonly BOOTOUT_IN_PROGRESS_STATUS=36
readonly HEALTH_TIMEOUT_SECONDS=30

function require_deploy_branch {
    local current_branch
    current_branch=$(git branch --show-current)

    if test "${current_branch}" != "${DEPLOY_BRANCH}"; then
        fail "deploy runs only on ${DEPLOY_BRANCH}; the current branch is ${current_branch:-a detached HEAD}"
    fi
}

function pull_and_rerun {
    git pull --ff-only

    exec ./scripts/deploy/deploy.sh "${AFTER_PULL_OPTION}"
}

function create_environment_file {
    if test -f ./.env; then
        return 0
    fi

    cp "${PRODUCTION_TEMPLATE_PATH}" ./.env
    echo "created ./.env from ${PRODUCTION_TEMPLATE_PATH}"
}

function require_production_settings {
    local problem_count=0

    for variable_name in "${PRODUCTION_VARIABLES[@]}"; do
        local variable_value
        variable_value=$(read_environment_value "${variable_name}")

        if test -z "${variable_value}"; then
            echo "error: ${variable_name} is not set in ./.env; the production settings are in ${PRODUCTION_TEMPLATE_PATH}" >&2
            problem_count=$((problem_count + 1))
            continue
        fi

        case "${variable_value}" in
            *"<"*">"*)
                echo "error: ${variable_name} in ./.env still holds a placeholder (${variable_value}); fill it in" >&2
                problem_count=$((problem_count + 1))
                ;;
        esac
    done

    if test "${problem_count}" -ne 0; then
        exit 1
    fi
}

function install_launch_agent_plist {
    local repository_root="$1"

    local log_directory plist_directory
    log_directory=$(dirname "${SERVER_LOG_PATH}")
    plist_directory=$(dirname "${LAUNCH_AGENT_PLIST_PATH}")
    mkdir -p "${log_directory}" "${plist_directory}"

    local staged_plist_path
    staged_plist_path=$(mktemp "${TMPDIR:-/tmp}/${LAUNCH_AGENT_LABEL}.XXXXXX")
    write_launch_agent_plist "${staged_plist_path}" "${repository_root}" "${SERVER_LOG_PATH}"

    local lint_output
    local lint_status=0
    lint_output=$(plutil -lint "${staged_plist_path}" 2>&1) || lint_status=$?

    if test "${lint_status}" -ne 0; then
        fail "the generated LaunchAgent property list is invalid; ${lint_output}"
    fi

    mv "${staged_plist_path}" "${LAUNCH_AGENT_PLIST_PATH}"
    echo "installed ${LAUNCH_AGENT_PLIST_PATH}"
}

function stop_launch_agent {
    local launch_agent_state
    launch_agent_state=$(read_launch_agent_state)

    if test "${launch_agent_state}" != "loaded"; then
        return 0
    fi

    local service_target
    service_target=$(read_launch_agent_service_target)

    local bootout_output
    local bootout_status=0
    bootout_output=$(launchctl bootout "${service_target}" 2>&1) || bootout_status=$?

    if test "${bootout_status}" -ne 0 && test "${bootout_status}" -ne "${BOOTOUT_IN_PROGRESS_STATUS}"; then
        fail "launchctl bootout ${service_target} failed with status ${bootout_status}; ${bootout_output}"
    fi

    local deadline=$((SECONDS + BOOTOUT_TIMEOUT_SECONDS))

    while test "${SECONDS}" -lt "${deadline}"; do
        launch_agent_state=$(read_launch_agent_state)

        if test "${launch_agent_state}" != "loaded"; then
            return 0
        fi

        sleep 1
    done

    fail "${LAUNCH_AGENT_LABEL} is still loaded ${BOOTOUT_TIMEOUT_SECONDS} s after launchctl bootout"
}

function start_launch_agent {
    local launch_agent_domain service_target
    launch_agent_domain=$(read_launch_agent_domain)
    service_target=$(read_launch_agent_service_target)

    launchctl bootstrap "${launch_agent_domain}" "${LAUNCH_AGENT_PLIST_PATH}"
    launchctl kickstart -k "${service_target}"
    echo "started ${service_target}"
}

function wait_for_health {
    local port
    port=$(read_environment_value BACTER_PORT)

    local deadline=$((SECONDS + HEALTH_TIMEOUT_SECONDS))
    local health_status_code="000"

    while test "${SECONDS}" -lt "${deadline}"; do
        health_status_code=$(read_health_status_code "${port}")

        if test "${health_status_code}" = "${HEALTHY_STATUS_CODE}"; then
            echo "healthy: http://127.0.0.1:${port}/health answered ${HEALTHY_STATUS_CODE}"
            return 0
        fi

        sleep 1
    done

    fail "http://127.0.0.1:${port}/health did not answer ${HEALTHY_STATUS_CODE} within ${HEALTH_TIMEOUT_SECONDS} s (last status ${health_status_code}); see ${SERVER_LOG_PATH} and ./scripts/deploy/server-status.sh"
}

function main {
    local repository_root
    repository_root=$(git rev-parse --show-toplevel)
    cd "${repository_root}"

    source ./scripts/git/git-preconditions.sh
    source ./scripts/deploy/deploy-shared.sh
    source ./scripts/build/program-paths.sh

    local pull_state="pending"

    case "${1:-}" in
        "") ;;
        "${AFTER_PULL_OPTION}") pull_state="done" ;;
        *)
            echo "usage: ./scripts/deploy/deploy.sh" >&2
            exit "${USAGE_EXIT_STATUS}"
            ;;
    esac

    require_deploy_branch
    require_clean_working_tree

    if test "${pull_state}" = "pending"; then
        pull_and_rerun
    fi

    create_environment_file
    require_production_settings

    ./setup.sh

    # setup.sh's PATH additions stay in its own process.
    add_homebrew_programs_to_path
    add_cargo_programs_to_path

    ./scripts/build/build-release.sh

    install_launch_agent_plist "${repository_root}"
    stop_launch_agent
    start_launch_agent
    wait_for_health
}

main "$@"
