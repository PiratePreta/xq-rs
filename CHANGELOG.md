# Changelog

All notable changes to the XQuad toolchain are recorded here. The
format loosely follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning is [SemVer](https://semver.org/) across both the Rust
crates and the Python distributions (single version string across the
whole workspace).

## [Unreleased]

_Nothing yet after v0.1.0._

## [0.1.0] — YYYY-MM-DD

First public release of the unified XQuad toolchain — the end state
of the xq-rs ↔ xq-py merge (QUI-412). Everything below was previously
available only to in-repo consumers; this is the first tag published
to crates.io and PyPI.

### Published artefacts

Three Rust crates on **crates.io**:

- `xqvm` — X-Quadratic Virtual Machine interpreter + bytecode types
  (`no_std + alloc` compatible for WASM embedding).
- `xqasm` — `.xqasm` text-format assembler and codec.
- `xqcli` — `xquad` command-line binary (`xquad run / asm / dsm`).

Five Python distributions on **PyPI**:

- `xqffi` — PyO3 FFI bindings exposing `xqvm` + `xqasm` to Python
  (`xqffi.vm`, `xqffi.asm`).
- `xqvm_py` — pure-Python reference VM used as the conformance
  oracle.
- `xqcp` — constraint-programming DSL compiling to `.xqasm`.
- `xqsa` — solver adapters (dwave-neal today; pluggable backend
  protocol).
- `xquad` — primary user-facing package. Owns the interactive
  `Program` / `Session` / `RunResult` API, a unified `VM` wrapper
  with backend dispatch, and re-exports the peer packages.

### Added

- **Spec** (`spec/xqvm/SPEC.md`) as the single source of truth for
  VM behaviour. Includes the opcode table, stack discipline, register
  model, error taxonomy, and HLF expansions.
- **Conformance harness** (`conformance/`) — cross-impl parity test
  suite. Every committed vector runs on both `xqvm` (Rust) and
  `xqvm_py` (Python) in CI; disagreement fails the build.
  `conformance/opcodes.yaml` is the machine-readable opcode table
  that both implementations are checked against
  (`scripts/check-opcode-parity.py`; `xqvm/build.rs`).
- **Interactive Python API** (`xquad.program`) — `Program.load()` /
  `Program.from_source()`, `Session` for multi-run workflows,
  `RunResult` with dict-keyed outputs (unset slots present as
  `None`). Built on top of `xqffi.asm` + `xqffi.vm`.
- **Umbrella distribution** (`xquad`) — `pip install xquad` pulls
  the full pipeline and exposes the interactive API
  (`xquad.program`), the unified `VM` wrapper (`xquad.vm`), canonical
  type aliases (`xquad.types`), plus re-exports of the peer packages
  so callers work with a single set of imports.
- **Examples** — `examples/tsp` and `examples/maxcut` end-to-end
  runners (xqcp → xqffi → xqsa) with committed `golden.json`
  fixtures.
- **Docs** — mdBook under `docs/book/`, Python API walkthrough at
  `docs/python-api-walkthrough.md`, bytecode semantics reference at
  `docs/bytecode-semantics.md`, per-package READMEs for every
  distribution.
- **Release pipeline** (`.gitlab/ci/release.yml`, `RELEASING.md`) —
  tag-triggered publishing to crates.io + PyPI with per-MR dry-runs.
- **Atomic spec-MR development discipline**
  ([`docs/xquad-development-workflow.md`](docs/xquad-development-workflow.md),
  [`scripts/check-atomic-spec-mr.sh`](scripts/check-atomic-spec-mr.sh))
  — VM-semantics MRs must touch `spec` + `xqvm` + `xqvm_py` +
  `conformance` in the same MR. Enforced by the
  `lint:atomic-spec-mr` CI job; exempt one-sided fixes via an
  `Atomic-Spec-Exempt: <reason>` commit trailer.

### Changed

- Python extension module renamed `xqapi_py` → `xqffi` and scoped
  to pure FFI (`xqffi.asm`, `xqffi.vm`); user-facing conveniences
  (`Program`, `Session`, `RunResult`) moved to the `xquad` umbrella
  package (QUI-464).
- `Vm.outputs()` is sparse: unset slots surface as `None` / absent
  entries instead of being coerced to `Int(0)` (QUI-459).
- SSMX / BSMX / XSMX samples are dense at allocation time and use
  domain-default values (spin → -1, binary → 0, discrete → 0);
  both impls now agree (QUI-453).

### Packaging

- `xqvm/opcodes.yaml` symlink → `../conformance/opcodes.yaml`.
  Keeps the conformance copy authoritative while letting
  `cargo publish` ship a self-contained tarball.
- Workspace dependencies (`xqvm`, `xqasm`) carry explicit version
  specifiers alongside `path`, as `cargo publish` requires.

### Known limitations (tracked separately)

- **Wheel matrix** is `linux-x86_64` only; macOS-arm64,
  linux-aarch64, and windows-x86_64 wheels require CI runner
  fan-out (QUI-442 follow-up).
- **Experimentation-branch promotion rule** not yet enforced by CI;
  documented in `AGENTS.md` but `scripts/check-atomic-spec-mr.sh`
  is planned for the next release (QUI-442 follow-up).
- **Example-smoke CI job** (`conformance:examples`) is temporarily
  disabled while a Python↔Rust cross-platform divergence is
  investigated (QUI-465).

[Unreleased]: https://gitlab.com/quip.network/xq-rs/-/compare/v0.1.0...main
[0.1.0]: https://gitlab.com/quip.network/xq-rs/-/releases/v0.1.0
