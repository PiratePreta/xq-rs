#!/usr/bin/env bash
# Copyright (C) 2026 Postquant Labs Incorporated
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Install the cargo-published dev tools the workspace depends on,
# pinned to the versions in scripts/cargo-tools.lock.
#
# Prefers `cargo binstall` (fetches prebuilt binaries — seconds)
# and falls back to `cargo install --locked` (compiles from
# source — minutes) when no prebuilt is available.
#
# Short-circuits when a binary of the right version is already on
# PATH, so CI cache hits skip straight through.
#
# Usage:
#   scripts/install-cargo-tools.sh [--upgrade]
#
# With --upgrade, every entry in scripts/cargo-tools.lock is
# reinstalled regardless of current state (useful after a lock-file
# bump).

set -euo pipefail

LOCK="$(cd "$(dirname "$0")" && pwd)/cargo-tools.lock"
UPGRADE=0

for arg in "$@"; do
    case "$arg" in
        --upgrade) UPGRADE=1 ;;
        *) echo "error: unknown arg: $arg" >&2; exit 2 ;;
    esac
done

# Bootstrap cargo-binstall via the upstream prebuilt-binary script.
# A `cargo install --locked cargo-binstall` bootstrap would compile
# from source (2–3 min on cold CI runners) — the exact slow path
# `binstall` exists to avoid. The upstream script fetches a release
# binary directly (seconds).
ensure_binstall() {
    if command -v cargo-binstall >/dev/null 2>&1; then
        return
    fi
    echo ">> installing cargo-binstall (upstream prebuilt bootstrap)…"
    curl -L --proto '=https' --tlsv1.2 -sSf \
        https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh \
        | bash
}

# Install one tool at the pinned version. Argument is a single
# `name=version [binary]` line from the lock file; we split the
# binary override off here.
install_one() {
    local name version binary already
    # shellcheck disable=SC2086 # intentional word-split on the spec.
    set -- $1
    local spec="$1" bin_override="${2:-}"
    name="${spec%=*}"
    version="${spec#*=}"
    binary="${bin_override:-$name}"

    if [[ "${UPGRADE}" -eq 0 ]] && command -v "${binary}" >/dev/null 2>&1; then
        already="$(${binary} --version 2>&1 | head -1 || true)"
        # The version-match heuristic is loose on purpose — some tools
        # print `cargo-deny 0.19.4`, others `taplo 0.10.0`, others
        # `mdbook v0.5.2`. We just look for the version substring.
        if [[ "${already}" == *"${version}"* ]]; then
            echo "   ${binary} @ ${version} already installed"
            return
        fi
    fi

    ensure_binstall
    echo ">> installing ${name}@${version}…"
    # --strategies=crate-meta-data guards against picking up wrong
    # binaries when crate name ≠ binary name. --locked mirrors the
    # cargo-install-from-source fallback behaviour.
    cargo binstall --no-confirm --locked "${name}@${version}"
}

# Walk the lock file, skipping empty lines and comments.
while IFS= read -r raw; do
    line="${raw%%#*}"
    line="${line#"${line%%[![:space:]]*}"}"
    line="${line%"${line##*[![:space:]]}"}"
    [[ -z "${line}" ]] && continue
    install_one "${line}"
done < "${LOCK}"

echo ">> cargo tools ready"
