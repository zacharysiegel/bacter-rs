# Git workflow

Governance for branches, commits, pull requests and integration. Each rule names the script which implements it, where one exists.

## Default branch

- `master`, locally and on GitHub.
- Scripts read it from `origin/HEAD` (`git symbolic-ref --short refs/remotes/origin/HEAD`). A repository whose origin was added with `git remote add` rather than cloned sets it once with `git remote set-head origin --auto`.

## Branches

- One short-lived branch per body of work, from the latest default branch.
- Serial phases of one plan are stacked: each phase branches from the previous phase's branch, and each pull request targets its parent branch.
- The first commit on every branch is the empty marker `>>> branch: <name>`, created by `./scripts/git/branch-init.sh <name>`. Integration rebases, so the markers are what keeps pull request boundaries visible in the default branch's history: `git log --grep '^>>> branch:'`.

## Commits

- One-line messages, no body, no attribution lines.
- Stage by explicit path; never `git add -A` or `git add .`; check `git status` before committing.
- Commit after each phase of a change; add a commit for a cleanup rather than amending.
- Every commit is pushed at once.

## Pull requests

- `gh pr create --assignee @me`, targeting the parent branch.
- The description is a release note: the problem and the solution in a few sentences, no test plan.

## Integration

- By local rebase through `./scripts/git/pr-integrate.sh <branch>` (or `--current`), never the GitHub rebase button, which drops the empty marker commits.
- A stacked branch whose parent already merged: `./scripts/git/pr-integrate.sh <branch> --from <former-parent>`, where `<former-parent>` is the parent branch or its former tip commit.
- Force pushes only with `--force-with-lease`.
- Merged branches are removed with `./scripts/git/cleanup-merged.sh <branch>...`; `pr-integrate.sh` runs it for the branch it integrates.

## Script registry

- `scripts/git/branch-init.sh`: a branch from HEAD with the marker commit, pushed with upstream tracking.
- `scripts/git/pr-integrate.sh`: integration by local rebase, fast-forward of the default branch, removal of the branch.
- `scripts/git/cleanup-merged.sh`: removal of merged branches from origin and locally, then `git remote prune origin`.
- `scripts/git/git-preconditions.sh`: the precondition checks the scripts above source; not run directly.
