#!/usr/bin/env bash
# Integrates a pushed branch into the default branch by local rebase, keeping its `>>> branch: <name>` marker
# commit (GitHub's rebase button drops empty commits), then removes the branch.
#
# Usage:
#   ./scripts/git/pr-integrate.sh <branch>
#   ./scripts/git/pr-integrate.sh --current
#   ./scripts/git/pr-integrate.sh <branch> --from <former-parent-branch>
#
# --current integrates the checked-out branch. --from rebases a stacked branch whose parent already merged:
# only the commits after <former-parent-branch> move onto the default branch.

set -euo pipefail

source "$(dirname "$0")/git-preconditions.sh"

function print_usage {
    echo "usage: $0 (<branch> | --current) [--from <former-parent-branch>]" >&2
}

function print_help {
    awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"
}

function read_current_branch {
    local current_branch
    current_branch=$(git symbolic-ref --short HEAD 2> /dev/null || true)

    if test -z "${current_branch}"; then
        echo "error: --current needs HEAD on a branch, not detached" >&2
        exit 1
    fi

    echo "${current_branch}"
}

function require_integrable_branch {
    local branch_name="$1"
    local default_branch="$2"

    if test "${branch_name}" = "${default_branch}"; then
        echo "error: refusing to integrate the default branch '${default_branch}' into itself" >&2
        exit 1
    fi

    local local_branch
    local_branch=$(local_branch_exists "${branch_name}")

    if test "${local_branch}" = "absent"; then
        echo "error: the local branch '${branch_name}' does not exist" >&2
        exit 1
    fi

    local remote_branch
    remote_branch=$(remote_branch_exists "${branch_name}")

    if test "${remote_branch}" = "absent"; then
        echo "error: the remote branch 'origin/${branch_name}' does not exist; push it first" >&2
        exit 1
    fi
}

function require_resolvable_ref {
    local ref_name="$1"

    local resolved_commit
    resolved_commit=$(git rev-parse --verify --quiet "${ref_name}^{commit}" || true)

    if test -z "${resolved_commit}"; then
        echo "error: the former-parent ref '${ref_name}' does not resolve to a commit" >&2
        exit 1
    fi
}

function integrate {
    local branch_name="$1"
    local default_branch="$2"
    local former_parent_branch="$3"

    echo ">>> Updating ${default_branch}"
    git checkout "${default_branch}"
    git pull --ff-only

    echo ">>> Rebasing '${branch_name}' onto ${default_branch}"
    git checkout "${branch_name}"

    if test -n "${former_parent_branch}"; then
        git rebase --onto "${default_branch}" "${former_parent_branch}" "${branch_name}"
    else
        git rebase "${default_branch}"
    fi

    echo ">>> Force-pushing the rebased '${branch_name}'"
    git push --force-with-lease

    echo ">>> Fast-forwarding ${default_branch} to '${branch_name}'"
    git checkout "${default_branch}"
    git rebase "${branch_name}"

    echo ">>> Pushing ${default_branch}"
    git push origin "${default_branch}"

    echo ">>> Removing '${branch_name}'"
    "$(dirname "$0")/cleanup-merged.sh" "${branch_name}"

    echo ">>> Done"
}

function main {
    local branch_name=""
    local former_parent_branch=""
    local branch_source="argument"

    while test "$#" -gt 0; do
        case "$1" in
            --from)
                if test "$#" -lt 2; then
                    echo "error: --from needs a former-parent branch" >&2
                    exit "${USAGE_EXIT_STATUS}"
                fi

                former_parent_branch="$2"
                shift 2
                ;;
            --current)
                branch_source="current"
                shift
                ;;
            -h|--help)
                print_help
                exit 0
                ;;
            -*)
                echo "error: unknown flag: $1" >&2
                exit "${USAGE_EXIT_STATUS}"
                ;;
            *)
                if test -n "${branch_name}"; then
                    echo "error: unexpected argument: $1" >&2
                    exit "${USAGE_EXIT_STATUS}"
                fi

                branch_name="$1"
                shift
                ;;
        esac
    done

    if test "${branch_source}" = "current" && test -n "${branch_name}"; then
        echo "error: --current cannot be combined with a branch argument" >&2
        exit "${USAGE_EXIT_STATUS}"
    fi

    require_git_repository

    if test "${branch_source}" = "current"; then
        branch_name=$(read_current_branch)
    fi

    if test -z "${branch_name}"; then
        print_usage
        exit "${USAGE_EXIT_STATUS}"
    fi

    local default_branch
    default_branch=$(read_default_branch)

    require_clean_working_tree
    require_integrable_branch "${branch_name}" "${default_branch}"

    if test -n "${former_parent_branch}"; then
        require_resolvable_ref "${former_parent_branch}"
    fi

    integrate "${branch_name}" "${default_branch}" "${former_parent_branch}"
}

main "$@"
