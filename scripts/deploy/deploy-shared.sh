# Settings and functions shared by the scripts in scripts/deploy/; sourced from the repository root, not run.

readonly LAUNCH_AGENT_LABEL="ro.zach.bacter.server"
readonly LAUNCH_AGENT_TEMPLATE_PATH="./scripts/deploy/${LAUNCH_AGENT_LABEL}.template.plist"
readonly LAUNCH_AGENT_PLIST_PATH="${HOME}/Library/LaunchAgents/${LAUNCH_AGENT_LABEL}.plist"
readonly SERVER_LOG_PATH="${HOME}/Library/Logs/bacter/server.log"
readonly HEALTHY_STATUS_CODE="200"

function fail {
    echo "error: $1" >&2
    exit 1
}

# Empty when ./.env does not set the variable; the caller's environment is ignored.
function read_environment_value {
    local variable_name="$1"

    (
        unset "${variable_name}"
        set -a
        source ./.env
        set +a
        printf '%s' "${!variable_name:-}"
    )
}

# gui/<uid> while the user has a GUI login session, otherwise user/<uid>.
function read_launch_agent_domain {
    local user_id
    user_id=$(id -u)

    local gui_domain="gui/${user_id}"
    local print_status=0
    launchctl print "${gui_domain}" > /dev/null 2>&1 || print_status=$?

    if test "${print_status}" -eq 0; then
        echo "${gui_domain}"
    else
        echo "user/${user_id}"
    fi
}

function read_launch_agent_service_target {
    local launch_agent_domain="$1"

    echo "${launch_agent_domain}/${LAUNCH_AGENT_LABEL}"
}

function read_launch_agent_state {
    local launch_agent_domain="$1"

    local service_target
    service_target=$(read_launch_agent_service_target "${launch_agent_domain}")

    local print_status=0
    launchctl print "${service_target}" > /dev/null 2>&1 || print_status=$?

    if test "${print_status}" -eq 0; then
        echo "loaded"
    else
        echo "not loaded"
    fi
}

# "000" when nothing answers.
function read_health_status_code {
    local port="$1"

    curl --silent --output /dev/null --max-time 2 --write-out '%{http_code}' "http://127.0.0.1:${port}/health" || true
}

function escape_sed_replacement {
    local replacement_text="$1"

    printf '%s' "${replacement_text}" | sed -e 's/[\\|&]/\\&/g'
}

function write_launch_agent_plist {
    local destination_path="$1"
    local repository_root="$2"
    local log_path="$3"

    local escaped_label escaped_repository_root escaped_log_path
    escaped_label=$(escape_sed_replacement "${LAUNCH_AGENT_LABEL}")
    escaped_repository_root=$(escape_sed_replacement "${repository_root}")
    escaped_log_path=$(escape_sed_replacement "${log_path}")

    sed \
        -e "s|__LAUNCH_AGENT_LABEL__|${escaped_label}|g" \
        -e "s|__REPOSITORY_ROOT__|${escaped_repository_root}|g" \
        -e "s|__LOG_PATH__|${escaped_log_path}|g" \
        "${LAUNCH_AGENT_TEMPLATE_PATH}" > "${destination_path}"
}
