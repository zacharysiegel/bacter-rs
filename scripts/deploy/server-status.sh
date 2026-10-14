#!/usr/bin/env bash
# Shows the server LaunchAgent's launchd state, the end of its log and the health check; exits non-zero unless healthy.
#
# Usage:
#   ./scripts/deploy/server-status.sh

set -euo pipefail

readonly LOG_LINE_COUNT=50

function print_launch_agent_state {
    local launch_agent_domain service_target
    launch_agent_domain=$(read_launch_agent_domain)
    service_target=$(read_launch_agent_service_target "${launch_agent_domain}")

    echo "== launchd: ${service_target}"

    local print_output
    local print_status=0
    print_output=$(launchctl print "${service_target}" 2>&1) || print_status=$?

    if test "${print_status}" -ne 0; then
        echo "not loaded"
        return 0
    fi

    echo "${print_output}"
}

function print_log_tail {
    echo "== log: ${SERVER_LOG_PATH} (last ${LOG_LINE_COUNT} lines)"

    if test ! -f "${SERVER_LOG_PATH}"; then
        echo "no log file"
        return 0
    fi

    tail -n "${LOG_LINE_COUNT}" "${SERVER_LOG_PATH}"
}

function print_health {
    local port
    port=$(read_environment_value BACTER_PORT)

    if test -z "${port}"; then
        fail "BACTER_PORT is not set in ./.env"
    fi

    local health_url="http://127.0.0.1:${port}/health"
    local health_status_code
    health_status_code=$(read_health_status_code "${port}")

    echo "== health: ${health_url}"

    if test "${health_status_code}" != "${HEALTHY_STATUS_CODE}"; then
        echo "unhealthy (status ${health_status_code})"
        exit 1
    fi

    echo "healthy (status ${health_status_code})"
}

function main {
    local repository_root
    repository_root=$(git rev-parse --show-toplevel)
    cd "${repository_root}"

    source ./scripts/deploy/deploy-shared.sh

    if test ! -f ./.env; then
        fail "./.env is missing; run ./scripts/deploy/deploy.sh first"
    fi

    print_launch_agent_state
    print_log_tail
    print_health
}

main
