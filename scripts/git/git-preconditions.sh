# Precondition checks shared by the scripts in scripts/git/; sourced, not run.

readonly USAGE_EXIT_STATUS=64

# Exit status of `git ls-remote --exit-code` when no ref matches; other non-zero statuses are failures.
readonly LS_REMOTE_NO_MATCH_STATUS=2

function require_git_repository {
    local git_directory_status=0
    git rev-parse --git-dir > /dev/null 2>&1 || git_directory_status=$?

    if test "${git_directory_status}" -ne 0; then
        echo "error: not inside a git repository" >&2
        exit 1
    fi
}

function require_clean_working_tree {
    # Non-zero means some files differ from the index, which diff-index reports below.
    local index_refresh_status=0
    git update-index -q --refresh > /dev/null || index_refresh_status=$?

    local working_tree_status=0
    git diff-index --quiet HEAD -- || working_tree_status=$?

    if test "${working_tree_status}" -ne 0; then
        echo "error: the working tree has uncommitted changes; commit or stash them first" >&2
        exit 1
    fi
}

function read_default_branch {
    local default_ref
    default_ref=$(git symbolic-ref --short refs/remotes/origin/HEAD 2> /dev/null || true)

    if test -z "${default_ref}"; then
        echo "error: origin/HEAD is not set; run: git remote set-head origin --auto" >&2
        exit 1
    fi

    echo "${default_ref#origin/}"
}

function remote_branch_exists {
    local branch_name="$1"

    local ls_remote_status=0
    git ls-remote --exit-code origin "refs/heads/${branch_name}" > /dev/null || ls_remote_status=$?

    if test "${ls_remote_status}" -eq 0; then
        echo "present"
    elif test "${ls_remote_status}" -eq "${LS_REMOTE_NO_MATCH_STATUS}"; then
        echo "absent"
    else
        echo "error: could not list the branches of origin; [status=${ls_remote_status}]" >&2
        exit 1
    fi
}

function local_branch_exists {
    local branch_name="$1"

    local show_ref_status=0
    git show-ref --verify --quiet "refs/heads/${branch_name}" || show_ref_status=$?

    if test "${show_ref_status}" -eq 0; then
        echo "present"
    else
        echo "absent"
    fi
}
