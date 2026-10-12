#!/usr/bin/env bash
# Checks that a site tree is complete: the prerendered document, and every /pkg/ asset it references on disk
# under a content-hashed name.
#
# Usage:
#   ./scripts/build/verify-site-tree.sh [<site-directory>]

set -euo pipefail

# cargo-leptos inserts a base64url digest between the stem and the extension.
readonly HASHED_NAME_PATTERN='\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9]+$'

function fail {
    echo "error: $1" >&2
    exit 1
}

function main {
    local repository_root
    repository_root=$(git rev-parse --show-toplevel)

    local site_directory="${1:-${repository_root}/target/site}"
    local document_path="${site_directory}/index.html"

    if test ! -d "${site_directory}"; then
        fail "${site_directory} does not exist; run ./scripts/build/build-site.sh"
    fi

    if test ! -f "${document_path}"; then
        fail "${document_path} is missing, so the site would serve no document at /"
    fi

    local referenced_paths
    referenced_paths=$(grep -o '/pkg/[A-Za-z0-9._-]*' "${document_path}" | sort -u || true)

    local reference_count=0

    for referenced_path in ${referenced_paths}; do
        if test ! -f "${site_directory}${referenced_path}"; then
            fail "the document references ${referenced_path}, which is not in ${site_directory}"
        fi

        local hashed_name
        hashed_name=$(printf '%s' "${referenced_path}" | grep -E "${HASHED_NAME_PATTERN}" || true)

        # The server caches /pkg/* as immutable, which is right only while a rebuild changes the name.
        if test -z "${hashed_name}"; then
            fail "the document references ${referenced_path}, whose name carries no content hash; build with ./scripts/build/build-site.sh"
        fi

        reference_count=$((reference_count + 1))
    done

    if test "${reference_count}" -eq 0; then
        fail "the document references no /pkg/ assets, so it would load nothing"
    fi

    echo "${site_directory}: prerendered document present, ${reference_count} referenced assets present and content-hashed"
}

main "$@"
