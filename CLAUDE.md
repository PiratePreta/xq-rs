# Claude Guidelines (xquad workspace root)

These guidelines cover the Python side of the workspace. For broader
agent context see [`AGENTS.md`](AGENTS.md); the merger of these two
into a single source is pending.

## Python workflow preferences

- **Dependencies:** manage via each package's `pyproject.toml`. The
  repo-root `pyproject.toml` hosts the `uv` workspace declaration and
  dev-tool pins (maturin, pytest, pyyaml, ruff); the xqvm_py / xqcp /
  xqsa / xqapi members carry their own. Never modify the dev-dep
  pins without explicit user approval.
- **Virtual environment:** always use the workspace `.venv/` managed
  by `uv sync` / `uv run`. Never install packages globally or create
  ad-hoc venvs. Invoke scripts and tests via `uv run …` so the
  maturin-built `xqapi_py` extension is picked up without a manual
  activation step.
- **Formatting:** all Python code must pass `ruff check .` and
  `ruff format --check .` (config in the root `pyproject.toml` under
  `[tool.ruff]`). After creating or modifying Python files run
  `uv run ruff check --fix <file>` and `uv run ruff format <file>`.
- **File changes:** aggregate edits to a single file into one pass.
  Do not thrash multiple small edits to the same file in sequence.

## References

- **Spec (`spec/xqvm/SPEC.md`):** authoritative source for XQVM
  architecture. Read the relevant sections before modifying VM
  behaviour. Spec changes are governed by the conformance harness at
  `conformance/`: any modification affecting the opcode table,
  control-flow rules, stack depth, type system, or HLF expansions must
  be mirrored in `conformance/opcodes.yaml` and validated against
  `xqvm_py/opcodes.py` via `scripts/check-opcode-parity.py`.
- **Conformance vectors (`conformance/vectors/`):** behavioural parity
  between `xqvm_py` (Python reference) and the Rust `xqvm` crate is
  enforced by the `xquad-conformance` test suite. New semantics
  require a new vector; divergence between impls fails CI with no
  drift-tracking middle ground.
- **Rust ↔ Python bindings (`xqapi/`):** the pyo3 crate exposes
  `xqasm` (`parse_xqasm`, `assemble_source`, `disassemble`) and
  `xqvm::Vm` to Python as `xqapi_py.asm` and `xqapi_py.vm`. `xqvm_py`
  consumes `xqapi_py.asm` only — its executor stays pure-Python so
  `xqvm_py` remains an independent conformance oracle.
