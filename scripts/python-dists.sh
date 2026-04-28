#!/usr/bin/env bash
# Copyright (C) 2026 Postquant Labs Incorporated
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Build and validate (or build and publish) the xquad Python
# distributions. Single source of truth for the peer/umbrella package
# list and the build/check/upload commands so the three CI jobs that
# touch them (release:dry-run:pypi, release:validate, release:publish-
# pypi) don't drift -- adding or renaming a package is a one-line
# edit here.
#
# Modes:
#   check    -- maturin build (xqffi cdylib) + uv build (peers) +
#               twine check. No network upload, no token needed. Used
#               by release:dry-run:pypi (MR/push) and release:validate
#               (tag).
#   publish  -- maturin publish (xqffi) + uv build + twine upload
#               (peers). Requires PYPI_TOKEN in env. Used by
#               release:publish-pypi (tag only).
#
# Run from the workspace root.

set -euo pipefail

# Pure-Python peers + umbrella, in any order: each is its own sdist.
# The pyo3 cdylib (xqffi) is handled separately because it ships as
# a maturin-built wheel rather than an sdist.
PEERS=(xqvm_py xqcp xqsa xquad)

mode="${1:-}"
case "${mode}" in
    check | publish) ;;
    *)
        echo "usage: $0 {check|publish}" >&2
        exit 2
        ;;
esac

if [[ "${mode}" == "publish" ]]; then
    : "${PYPI_TOKEN:?PYPI_TOKEN is required for publish mode}"
fi

# --- xqffi (pyo3 cdylib) ---------------------------------------------------
if [[ "${mode}" == "check" ]]; then
    maturin build --release --manifest-path xqffi/Cargo.toml --out xqffi/dist
    twine check xqffi/dist/*
else
    maturin publish --manifest-path xqffi/Cargo.toml \
        --username __token__ --password "${PYPI_TOKEN}" --skip-existing
fi

# --- pure-Python peers + umbrella -----------------------------------------
# `uv build` inside a workspace member defaults to the workspace-root
# `dist/`; `--out-dir dist` keeps each package's artefacts under its own
# subdir so subsequent twine ops resolve files locally to that package.
for pkg in "${PEERS[@]}"; do
    (
        cd "${pkg}"
        uv build --out-dir dist
        if [[ "${mode}" == "publish" ]]; then
            twine upload --non-interactive --skip-existing \
                --username __token__ --password "${PYPI_TOKEN}" dist/*
        else
            twine check dist/*
        fi
    )
done
