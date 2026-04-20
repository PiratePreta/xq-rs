# XQSA Specification (draft)

**Status:** draft. Authoritative specification is pending formalisation of
the solver-adapter contract from the `xqvm-py/xqsa/` reference
implementation.

XQSA (X-Quadratic Solver Adapters) defines the interface between the
XQVM toolchain and external quadratic optimization solvers (simulated
annealing, quantum annealers, etc.). The goal is a single-sample-in /
energy-out contract so that any solver can be plugged into the XQVM
execution pipeline without touching the VM.

## Scope (forthcoming)

The specification will cover:

- Solver interface: `solve(model: XQMX, **kwargs) -> Sample`.
- Sample encoding: variable assignments compatible with
  [`../xqvm/SPEC.md`](../xqvm/SPEC.md) `Sample` register values.
- Energy contract: `energy(model, sample)` must agree to full precision
  with the `ENERGY` opcode on both Rust and Python implementations for
  every sample the solver returns.
- Capability negotiation: binary / spin / discrete domains, problem-size
  limits, determinism guarantees.

## Reference implementation

[`../../xqvm-py/xqsa/`](../../xqvm-py/xqsa/) — currently wraps `dwave-neal`
simulated annealing.
