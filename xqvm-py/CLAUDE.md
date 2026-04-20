# Claude Guidelines

## Preferences

- **File Changes:** Aggregate all changes for a file and apply them together. Do not make multiple small edits to the same file in sequence.
- **Dependencies:** Never add dependencies to `requirements.txt` without explicit user approval.
- **Virtual Environment:** Always use the local virtual environment defined in `.venv`. Never install packages globally or create new environments.
- **Formatting:** All Python code must pass `ruff check .` and `ruff format --check .` (config in `ruff.toml`). After creating or modifying Python files, run `ruff check --fix <file>` and `ruff format <file>`.

## References

- **Specification (`../spec/xqvm/SPEC.md`):** This is the authoritative source for XQVM architecture. Read relevant sections before modifying VM behaviour. Spec changes are now governed by the conformance harness (`../conformance/`): any modification that affects opcode table, control flow rules, stack depth, type system, or HLF expansions must be mirrored in `conformance/opcodes.yaml` and validated against `xqvm-py/xqvm/core/opcodes.py` via `scripts/check-opcode-parity.py`.
- **Conformance vectors (`../conformance/vectors/`):** Behavioural parity between this implementation and the Rust `xqvm` crate is enforced by the `xquad-conformance` test suite. New semantics require a new vector; divergence from the committed `expected.json` fails CI.
