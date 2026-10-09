#!/usr/bin/env bash
# Creates a branch from HEAD whose first commit is the empty marker `>>> branch: <name>`, and pushes it.
#
# Usage:
#   ./scripts/git/branch-init.sh <branch-name>

set -euo pipefail

source "$(dirname "$0")/git-preconditions.sh"

function main {
    if test "$#" -ne 1 || test -z "$1"; then
        echo "usage: $0 <branch-name>" >&2
        exit "${USAGE_EXIT_STATUS}"
    fi

    local branch_name="$1"

    require_git_repository
    require_clean_working_tree

    local local_branch
    local_branch=$(local_branch_exists "${branch_name}")

    if test "${local_branch}" = "present"; then
        echo "error: the local branch '${branch_name}' already exists" >&2
        exit 1
    fi

    local remote_branch
    remote_branch=$(remote_branch_exists "${branch_name}")

    if test "${remote_branch}" = "present"; then
        echo "error: the remote branch 'origin/${branch_name}' already exists" >&2
        exit 1
    fi

    git checkout -b "${branch_name}"
    git commit --allow-empty -m ">>> branch: ${branch_name}"
    git push -u origin "${branch_name}"
}

main "$@"
