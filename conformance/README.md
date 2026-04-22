# XQVM Conformance Harness

Mechanical cross-implementation check that the Rust production VM
([`xqvm`](../xqvm/)) and the Python reference VM
([`xqvm_py`](../xqvm_py/)) agree on every observable behaviour defined
by [`spec/xqvm/SPEC.md`](../spec/xqvm/SPEC.md).

The harness is a Rust test crate (`xquad-conformance`). Each vector
becomes two `#[test]` functions — one per runtime — generated at
`cargo build` time by [`build.rs`](build.rs). CI runs them as two
independent GitLab jobs so a Python-side regression cannot mask a
Rust-side pass (or vice versa).

## Layout

```
conformance/
├── opcodes.yaml          single machine-readable source for the 87-opcode table
├── vectors/
│   ├── arithmetic/<name>/
│   │   ├── program.xqasm       canonical assembly source
│   │   ├── program.xqb         committed bytecode (re-assembled and cmp'd by the harness)
│   │   ├── inputs.json         {"calldata": [i64, ...], "output_slots": N}
│   │   └── expected.json       {"outputs": [i64|null, ...], "final_stack": [i64, ...]}
│   ├── control-flow/<name>/
│   ├── energy/<name>/
│   └── constraints/<name>/
├── src/                  Rust library + manual CLI binary
├── tests/rust.rs         generated in-process Rust runner tests
└── tests/python.rs       generated subprocess Python runner tests
```

## File format

### `program.xqasm`

Canonical human-readable source. The assembler is authoritative: whatever
`xqasm::assemble_source()` produces becomes the canonical bytecode.
Prefer explicit `PUSH1`/`PUSH2`/… forms when the constant width matters
for the vector; use the `PUSH` sugar where it doesn't.

### `program.xqb`

Committed bytecode. The harness re-assembles `program.xqasm` on every
run and `cmp`'s against this file via
[`verify_bytecode_fresh`](src/lib.rs) — any mismatch fails the vector's
test. This catches accidental assembler drift that a pure-source harness
would miss.

Regenerate after changing the assembly source:

```sh
cargo run -p xqcli -- asm conformance/vectors/<cat>/<name>/program.xqasm \
    -o conformance/vectors/<cat>/<name>/program.xqb
```

### `inputs.json`

```json
{
  "calldata": [6, 7],
  "output_slots": 16
}
```

`output_slots` is optional (defaults to 16, matching `xquad run`). The
`calldata` array is exposed to the program in slot order: slot 0 holds
the first value, slot 1 the second, and so on.

### `expected.json`

```json
{
  "outputs": [42, 0, 0],
  "final_stack": []
}
```

Output slots that remain unset by the program are reported as `0`
(matches the Rust VM's `RegVal::default()` pre-fill — the VM cannot
currently distinguish "never written" from "explicitly zero").
Spec-compliant sparse reporting is tracked as a follow-up. The length
of `outputs` must equal `output_slots`. `final_stack` is the residual
stack at `HALT` (bottom to top); an empty stack is typical.

## Authoring a new vector

1. Write `program.xqasm` with a minimal scenario that exercises the
   behaviour you care about.
2. Pick the matching category directory (or create a new one; the
   category slug must match a section in `opcodes.yaml`).
3. Generate the bytecode:

   ```sh
   cargo run -p xqcli -- asm <dir>/program.xqasm -o <dir>/program.xqb
   ```

4. Write `inputs.json` with the calldata the program reads.
5. Produce `expected.json` by running one of the two impls and
   inspecting the result. The harness then asserts the *other* impl
   agrees. If they disagree, you've found spec drift — file a bug
   ticket, and either fix the divergent impl or exclude the vector
   with a comment referencing the ticket.

## Drift policy

There is no `DRIFT.md`. Either every vector passes on both runtimes or
the build is broken. Concrete enforcement:

- **Opcode table** — `xqvm/build.rs` asserts `opcodes.yaml ↔ opcodes!
  x-macro` at compile time; `scripts/check-opcode-parity.py` asserts
  `opcodes.yaml ↔ xqvm_py/opcodes.py` in CI.
- **Bytecode encoding** — [`verify_bytecode_fresh`](src/lib.rs) asserts
  `program.xqasm` round-trips to `program.xqb` on every vector run.
- **Observable behaviour** — [`check_vector`](src/lib.rs) asserts the
  observed `{outputs, final_stack}` matches `expected.json` for both
  runtimes.

## Running locally

```sh
# Full matrix (both runtimes, all vectors)
cargo test -p xquad-conformance

# Rust only
cargo test -p xquad-conformance --no-default-features --features rust

# Python only (needs python3 + xqvm_py importable from xqvm_py/)
cargo test -p xquad-conformance --no-default-features --features python

# Manual CLI for authoring / triaging a single vector
cargo run -p xquad-conformance -- --filter arithmetic/add_basic --impl both
```

Override the Python interpreter with `XQUAD_CONFORMANCE_PYTHON=...`.
