#!/usr/bin/env bash
# Copyright (C) 2026 Postquant Labs Incorporated
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Wrap `cargo publish --dry-run` for a workspace member, tolerating the
# specific "workspace dep not yet on crates.io" failure that occurs at
# first-publish of a multi-crate workspace.
#
# Why this wrapper exists
# -----------------------
# `cargo publish --dry-run` runs two phases:
#
#   1. Pack -- copy source files into a `.crate` tarball and resolve
#      dependencies against the registry to construct the resolved
#      manifest.
#   2. Verify -- extract the tarball and compile from the packaged
#      source. Skipped via `--no-verify`.
#
# Phase 1 still resolves deps even with `--no-verify`. For a workspace
# member like xqasm that depends on `xqvm = "0.1.0"`, that resolution
# fails at the first publish because xqvm hasn't been pushed to
# crates.io yet -- and won't be until `release:publish-crates` actually
# runs (which publishes xqvm first, then xqasm, then xqcli).
#
# This wrapper:
#   - Runs cargo publish --dry-run for the named crate.
#   - On exit 0: passes through.
#   - On exit non-zero with the specific "no matching package named
#     `<workspace-crate>`" error: logs the expected first-publish miss
#     and exits 0. The actual sequential publish handles ordering.
#   - On exit non-zero with any other error (metadata bugs, license
#     missing, etc.): exits with the original code, blocking the
#     pipeline.
#
# From v0.1.1 onwards (xqvm on crates.io), the dry-run resolves cleanly
# and the wrapper is just a pass-through. Same behaviour for the rest
# of the workspace's lifetime -- no follow-up needed.
#
# Usage:
#   scripts/validate-crate-publish.sh <crate-name> [extra cargo flags]
# Example:
#   scripts/validate-crate-publish.sh xqasm --no-verify

set -euo pipefail

crate="${1:?usage: $0 <crate-name> [extra cargo publish flags...]}"
shift

# Workspace member names. If cargo's "no matching package named ..."
# error names one of these, treat it as the expected first-publish
# miss. Adding a new workspace crate? Add it here too.
WORKSPACE_CRATES=(xqvm xqasm xqcli xqffi)

set +e
out=$(cargo publish --dry-run --locked "$@" -p "${crate}" 2>&1)
exit_code=$?
set -e

# Always echo the cargo output so the pipeline log is intact.
echo "${out}"

if [[ "${exit_code}" -eq 0 ]]; then
    exit 0
fi

# Match cargo's exact phrasing: `no matching package named \`<crate>\``
# Search for any workspace member name in that pattern.
for ws in "${WORKSPACE_CRATES[@]}"; do
    if echo "${out}" | grep -qE "no matching package named \`${ws}\`"; then
        echo
        echo "[validate-crate-publish] ${crate} dry-run failed because workspace dep '${ws}'"
        echo "                         is not yet on crates.io. This is expected at the first"
        echo "                         publish of a multi-crate workspace -- '${ws}' will be"
        echo "                         published before '${crate}' by release:publish-crates,"
        echo "                         which runs sequentially. Treating as success."
        exit 0
    fi
done

# Any other failure: real error -- propagate cargo's exit code.
echo
echo "[validate-crate-publish] ${crate} dry-run failed with an unexpected error" >&2
echo "                         (not a missing-workspace-crate first-publish miss)." >&2
exit "${exit_code}"
