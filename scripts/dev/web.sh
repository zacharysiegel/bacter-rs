#!/usr/bin/env bash
# Serves the site with cargo-leptos on the web crate's site-addr, rebuilding on change.

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
cd "${repository_root}/web"

exec cargo leptos watch
