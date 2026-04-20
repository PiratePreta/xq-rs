# XQCP Specification (draft)

**Status:** draft. Authoritative specification is pending formalisation of
the DSL semantics from the `xqvm-py/xqcp/` reference implementation.

XQCP (X-Quadratic Constraint Programming) is a Python-embedded DSL for
describing quadratic optimization problems symbolically and compiling them
to the three-program XQVM architecture (encoder, verifier, decoder).

## Scope (forthcoming)

The specification will cover:

- Symbolic value types (`Int`, `Vec`, `XQMX`) and their operator algebra.
- Problem lifecycle: `input` → `define_model` → `define_objective` →
  `define_constraints` → `define_outputs` → `compile`.
- Constraint taxonomy: one-hot, exclusion, implication, equality, linear.
- Compilation contract: for every well-formed XQCP program, the three
  emitted `.xqasm` programs must execute without error on any XQVM
  implementation conforming to [`../xqvm/SPEC.md`](../xqvm/SPEC.md).

## Reference implementation

[`../../xqvm-py/xqcp/`](../../xqvm-py/xqcp/) is the current reference. Any
formal specification here must be validated against it; if the two
diverge, the spec is authoritative and the reference is a bug.
