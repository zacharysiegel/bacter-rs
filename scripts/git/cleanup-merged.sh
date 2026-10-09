#!/usr/bin/env bash
# Deletes merged branches from origin and locally, then prunes stale remote-tracking refs.
# Deletes with -D: a branch integrated from another clone keeps a pre-rebase local tip here.
#
# Usage:
#   ./scripts/git/cleanup-merged.sh <branch-name> [<branch-name>...]

set -euo pipefail

source "$(dirname "$0")/git-preconditions.sh"

function require_deletable_branches {
    local default_branch="$1"
    shift

    local current_branch
    current_branch=$(git rev-parse --abbrev-ref HEAD)

    for branch_name in "$@"; do
        if test "${branch_name}" = "${default_branch}"; then
            echo "error: refusing to delete the default branch '${default_branch}'" >&2
            exit 1
        fi

        if test "${branch_name}" = "${current_branch}"; then
            echo "error: cannot delete '${branch_name}' while it is checked out" >&2
            exit 1
        fi
    done
}

function delete_remote_branches {
    local remote_branch_names=()

    for branch_name in "$@"; do
        local remote_branch
        remote_branch=$(remote_branch_exists "${branch_name}")

        if test "${remote_branch}" = "present"; then
            remote_branch_names+=("${branch_name}")
        fi
    done

    if test "${#remote_branch_names[@]}" -eq 0; then
        return 0
    fi

    local push_status=0
    git push origin --delete "${remote_branch_names[@]}" 2> /dev/null || push_status=$?

    if test "${push_status}" -eq 0; then
        return 0
    fi

    # GitHub's automatic head-branch deletion can remove a branch between the listing above and the push.
    echo "note: the delete push failed; checking whether origin already deleted the branches"
    git fetch --prune origin > /dev/null

    for branch_name in "${remote_branch_names[@]}"; do
        local remote_branch
        remote_branch=$(remote_branch_exists "${branch_name}")

        if test "${remote_branch}" = "present"; then
            echo "error: the branch '${branch_name}' still exists on origin" >&2
            exit 1
        fi
    done
}

function delete_local_branches {
    local local_branch_names=()

    for branch_name in "$@"; do
        local local_branch
        local_branch=$(local_branch_exists "${branch_name}")

        if test "${local_branch}" = "present"; then
            local_branch_names+=("${branch_name}")
        fi
    done

    if test "${#local_branch_names[@]}" -gt 0; then
        git branch -D "${local_branch_names[@]}"
    fi
}

function main {
    if test "$#" -eq 0; then
        echo "usage: $0 <branch-name> [<branch-name>...]" >&2
        exit "${USAGE_EXIT_STATUS}"
    fi

    require_git_repository

    local default_branch
    default_branch=$(read_default_branch)

    require_deletable_branches "${default_branch}" "$@"
    delete_remote_branches "$@"
    delete_local_branches "$@"

    git remote prune origin
}

main "$@"
