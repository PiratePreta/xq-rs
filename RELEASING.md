# Releasing the xquad toolchain

Cutting a release publishes eight artefacts in one shot from a single
`v<X.Y.Z>` git tag:

| # | Artefact | Registry | Ordering |
|---|----------|----------|----------|
| 1 | [`xqvm`](xqvm/) | crates.io | before xqasm, xqcli |
| 2 | [`xqasm`](xqasm/) | crates.io | before xqcli |
| 3 | [`xqcli`](xqcli/) | crates.io | last Rust crate |
| 4 | [`xqffi`](xqffi/) wheel | PyPI | before peers (pyo3 cdylib) |
| 5 | [`xqvm_py`](xqvm_py/) sdist | PyPI | before xquad |
| 6 | [`xqcp`](xqcp/) sdist | PyPI | before xquad |
| 7 | [`xqsa`](xqsa/) sdist | PyPI | before xquad |
| 8 | [`xquad`](xquad/) sdist | PyPI | last — depends on 4-7 |

Triggered by `.gitlab/ci/release.yml`; see that file for the exact
ordering and rules. `xqffi` the Rust *crate* stays `publish = false`
(cdylib-only, consumed via PyPI).

## Prerequisites (one-time)

Protected CI variables must be present in the GitLab project
(**Settings → CI/CD → Variables**, both masked + protected):

- `CARGO_REGISTRY_TOKEN` — crates.io API token from
  `ops@postquant.xyz`. Scope to `xqvm`, `xqasm`, `xqcli` publish-new
  + publish-update.
- `PYPI_TOKEN` — PyPI API token from `ops@postquant.xyz`. The first
  release needs an **account-wide** token because per-project tokens
  require the project to exist. After v0.1.0 lands, rotate to
  per-project tokens scoped to the five PyPI names.

Both accounts are owned by `ops@postquant.xyz` (QUI-436 placeholder
reservations done by Keith; real release happens when this pipeline
runs).

## Pre-flight

Before cutting a tag:

1. **Watch the dry-run jobs.** Every MR pipeline runs
   `release:dry-run:crates` and `release:dry-run:pypi` in the `lint`
   stage. If either is red on `main`, do not tag — the real release
   will fail the same way, just with partial artefacts already
   uploaded.
2. **Verify Substrate pallet coordination.** Crate renames force
   major-version bumps downstream. Ping the pallet team before the
   first `xqvm 0.1.0` release so their pins move atomically; for
   non-breaking bumps (`0.1.x → 0.1.y`) a ping is courtesy.
3. **Verify workspace version is bumped.** Every crate's
   `Cargo.toml` and every Python package's `pyproject.toml` must
   agree on the version being tagged.
4. **Update `CHANGELOG.md`** (when it exists — tracked as a later
   QUI-442 follow-up). At minimum for v0.1.0, write the first entry.

## Cutting a release

From a clean checkout of the commit you intend to release (typically
`main` at the tip, or a release branch like `release/0.1`):

```sh
# 1. Bump versions in every manifest.
#    (Until we scripted this, edit by hand. Touchpoints:
#     Cargo.toml workspace.package.version, each crate's Cargo.toml,
#     each pyproject.toml [project] version.)
git commit -s -am "chore: bump workspace to X.Y.Z"

# 2. Push the version bump and wait for CI to go green.
git push

# 3. Tag.
git tag -s vX.Y.Z -m "xquad vX.Y.Z"
git push origin vX.Y.Z
```

The tag push triggers `release:crates` (stage `release`), which on
success triggers `release:pypi` via `needs:`. Watch the pipeline.

If either stage fails mid-way, rerun **only the failed job** — both
jobs pass `--skip-existing` / `--locked` flags so re-runs are
idempotent. Do not retag unless the failure was a version mistake.

## Post-flight

1. **Verify on the registries.** All four crates (xqvm, xqasm, xqcli,
   and — eventually, once we publish it — xqffi's cdylib) should show
   `vX.Y.Z` within a minute of pipeline completion; all five Python
   distributions (`xqffi`, `xqvm_py`, `xqcp`, `xqsa`, `xquad`) on
   PyPI within seconds.
2. **Smoke-test the install.** In a fresh venv on your workstation:

   ```sh
   python3.13 -m venv /tmp/xquad-smoke
   source /tmp/xquad-smoke/bin/activate
   pip install "xquad==X.Y.Z"
   python -c "import xquad; from xquad import vm, asm; v = vm.Vm(); print('ok')"
   ```

3. **Notify the pallet team** if this was a major bump they're
   blocked on.

## Trouble-shooting

- **`cargo publish` fails with "version already exists":** someone
  already published that version. Either bump to the next one or
  retag to the existing commit on the registry side (rare).
- **`twine upload` fails with 400 File already exists:** partial
  previous upload. The `--skip-existing` flag should make re-runs a
  no-op; if not, check PyPI and either bump the version or delete the
  uploaded file (within 24h) and retry.
- **`release:pypi` runs but `pip install xquad` still fails:** PyPI
  index propagation can take a few minutes for the *first* release of
  a new package name. Retry after 5 min before digging further.
- **Dry-run passes on MR but real release fails:** usually means the
  release job has access to a token the MR pipeline doesn't. Check
  that the token variables have `Protected: yes` and that the tag is
  on a protected tag pattern (`v*`). Protected variables only expose
  to protected refs.

## What this pipeline does *not* do yet

- **Multi-platform wheels (user-facing limitation).**
  `pip install xquad` currently only works on `linux-x86_64`. macOS
  (Intel and ARM), linux-aarch64, and Windows users cannot install
  from PyPI — they must build from source with a local Rust toolchain
  (`maturin develop` + `uv sync`). There is no sdist fallback: the
  `xqffi` pyo3 cdylib that `xquad` depends on is wheel-only, so
  install does not degrade gracefully, it fails outright. The
  multi-arch wheel matrix requires runner fan-out to platform-specific
  runners — tracked as a QUI-442 follow-up.
- **Automated version bumps.** No `cargo-release` / `hatch version`
  integration yet; versions are edited by hand per the step above.
- **Release notes autogeneration.** CHANGELOG.md is maintained
  manually; conventional-commits-based generation is a future
  consideration.
- **Signed tags + signed artefacts.** Tags are expected to be git-
  signed (`git tag -s`); crates.io / PyPI artefact signing (sigstore
  cosign, PEP 740) is not wired. Tracked separately.
