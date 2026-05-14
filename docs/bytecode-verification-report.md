# Bytecode Verification -- Surface Area Report

**Date:** 2026-05-07
**Branch:** feature/qui-491-493
**Scope:** Pre-execution and compile-time verification opportunities for XQVM bytecode

---

## Executive Summary

The XQuad toolchain has a complete runtime error model (19 error variants in `xqvm::Error`,
mirrored in `xqvm_py`) but **no pre-execution verification layer**. Every type error, stack
underflow, unset register access, and malformed jump currently requires a live VM execution
to surface. This report enumerates all pipeline stages where static or structural checks
could catch errors earlier -- at assembly time, during bytecode loading, or in a dedicated
verifier pass -- without changing runtime semantics.

---

## 1. Pipeline Stages and Current State

```
xqcp (Python DSL)
  └─> xqasm (assembler)
        └─> codec::encode (binary bytecode)
              └─> [blockchain / storage]
                    └─> codec::decode
                          └─> Vm::run (execution)
```

Each stage is a verification opportunity. Today, only the assembler and the runtime perform
meaningful checks.

---

## 2. Binary Bytecode Format (codec.rs)

### What exists
- Opcode byte validity (`Opcode::try_from(u8)` at decode time).
- Truncated instruction detection (`Error::TruncatedInstruction`).

### What is missing
- **No magic bytes or version header.** A file silently accepted as valid bytecode may be
  arbitrary bytes or a different version.
- **No integrity check.** Bit-flips or partial writes are undetectable.
- **No structural metadata.** The expected INPUT count, OUTPUT count, and jump table size
  are not recorded, so the decoder cannot validate those constraints before execution.

### Proposed checks
| Check | Effort | Impact |
|---|---|---|
| Magic bytes `XQBC` + version `u8` | Low | Prevents version confusion, accidental misuse |
| CRC32 over code section | Low | Detects corruption before blockchain submission |
| `expected_inputs: u16`, `expected_outputs: u16` in header | Low | Enables pre-run slot validation |
| Jump table size hint | Low | Validates TARGET count at load time |

---

## 3. Assembler (xqasm/src/assembler.rs)

### What exists
- Mnemonic validation.
- Operand count and kind checking (register vs. integer vs. label).
- Register index range (0-255).
- Integer immediate range (e.g., PUSH1 fits i8).
- Label existence: duplicate label detection, unplaced label errors.

### What is missing
- **Register type tracking.** The assembler emits code like:
  ```
  PUSH 10
  STOW r0       ; r0 becomes Int
  BQMX r0       ; r0 becomes Model -- overwrites Int silently
  LOAD r0       ; expects Int; fails at runtime
  ```
  No warning is issued at assembly time.
- **Stack effect analysis.** The assembler does not compute stack depth at any instruction
  point, so underflows are only caught at runtime.
- **Loop structure validation.** RANGE/ITER without NEXT, or NEXT without a matching open
  loop, is not flagged.
- **Operand value range constraints.** XQMX `k < 2` and shift amounts outside 0-63 are not
  checked.

### Proposed checks
| Check | Effort | Impact |
|---|---|---|
| Register type state machine (forward pass) | Medium | Catches all `RegisterType` runtime errors |
| Stack depth tracking per instruction | Medium | Catches `StackUnderflow` / `StackOverflow` |
| RANGE/ITER -- NEXT nesting validation | Low | Catches `UnmatchedLoop` / `NoActiveLoop` |
| Operand value constraints (k >= 2, shift 0-63) | Low | Catches `InvalidDiscreteK`, `InvalidShift` |

---

## 4. InstructionBuilder (xqvm/src/bytecode/builder.rs)

### What exists
- Label placement and fixup resolution.
- Duplicate and unplaced label detection.
- `TooManyTargets` check (> u16::MAX targets).

### What is missing
- Identical gaps as the assembler: no type tracking, no stack effect analysis, no loop
  nesting check. Code that uses `InstructionBuilder` programmatically (e.g., test harnesses,
  future compiler backends) gets no validation.

### Proposed addition
A `verify()` method (or integrated into `build()`) that runs the structural checks above on
the finalized instruction sequence before encoding. This gives programmatic users the same
guarantees as the assembler.

---

## 5. Register Type System

### Current runtime model
`RegVal` in `xqvm/src/value.rs` defines six types:

| Variant | Written by | Read by |
|---|---|---|
| `Unset` | `DROP` or initial state | (error on any read) |
| `Int(i64)` | `STOW`, `INPUT` | `LOAD`, arithmetic, stack ops |
| `VecInt(Vec<i64>)` | `VEC`, `VECI`, `VECPUSH` (on VecInt) | `VECGET`, `VECSET`, `VECLEN`, `SLACK`, `REDUCE` |
| `VecXqmx(Vec<XqmxModel>)` | `VECX`, `VECPUSH` (on VecXqmx) | `GETLINE`, `SETLINE`, `ADDLINE`, etc. |
| `Model(XqmxModel)` | `BQMX`, `SQMX`, `XQMX` | all XQMX coeff/grid/HLF ops, `ENERGY` |
| `Sample(XqmxSample)` | `BSMX`, `SSMX`, `XSMX` | grid ops, `ENERGY` |

### Static verification algorithm

A single forward pass over the instruction sequence maintains:

```
type_state: [RegType; 256]  -- one entry per register slot, initial = Unset
```

For each instruction:
1. Check that every source register's entry in `type_state` matches the expected type.
   Emit a `VerificationError::RegisterTypeMismatch` if not.
2. Check that every source register is not `Unset`.
   Emit `VerificationError::UnsetRegister` if it is.
3. Update `type_state` for every destination register to the type written by this opcode.

For control flow (JUMP, JUMPI), the pass must propagate type state across join points --
taking the intersection (only registers whose type agrees on all incoming paths are
considered definitely typed). This is the "may-be-unset" approximation and matches what
the JVM bytecode verifier does.

### Errors statically detectable

| Runtime error | Statically detectable? |
|---|---|
| `RegisterType { reg, expected, got }` | Yes -- forward type pass |
| `UnsetRegister { pos, reg }` | Yes -- reaching definitions |
| `StackUnderflow { pos }` | Yes -- stack effect analysis |
| `StackOverflow { pos }` | Yes (worst-case depth) |
| `NoActiveLoop { pos }` | Yes -- nesting counter |
| `BadJumpTarget { pos, target }` | Yes -- jump target pre-scan |
| `InvalidLabel { pos, label }` | Yes -- label existence check |
| `TruncatedInstruction { pos }` | Yes -- codec level |
| `UnmatchedLoop { pos }` | Yes -- nesting counter |
| `InvalidDiscreteK { pos, k }` | Yes (constant operand) |
| `InvalidShift { pos, amount }` | Yes (constant operand) |
| `DivisionByZero` | No (runtime value) |
| `IndexOutOfBounds` | No (runtime value) |
| `SizeMismatch` | Partial (if sizes are static) |
| `StepLimitExceeded` | No |

---

## 6. Stack Effect Analysis

Each opcode has a fixed, statically-known stack delta. A verifier tracks the minimum and
maximum stack depth reachable at every instruction point:

- If `min_depth < 0` at any POP-consuming opcode: `StackUnderflow` is guaranteed.
- If `max_depth > 8192` at any push: `StackOverflow` is possible.

Loop bodies must have **zero net stack effect** (same depth on entry to NEXT as on entry
to RANGE/ITER); otherwise the loop accumulates or drains the stack unboundedly.

---

## 7. Python Compiler (xqcp)

### Current state
`compiler.py` maintains a `symbols.py` table tracking register allocation (InputRef,
ModelRef, OutputRef, LoopVar), but type consistency is implicit -- errors surface only
when generated assembly is executed.

### What is missing
- **Generated code is not validated before emission.** If an expression coercion in
  `expression.py` produces an incompatible register use, it goes undetected until runtime.
- **No vector length consistency check.** Constraint operations like EQUALITY, ATLEAST,
  and ATLEASTW take paired vector registers; their lengths must match. The compiler does
  not verify this.
- **No ENERGY operand compatibility check.** ENERGY takes a `(Model, Sample)` pair; the
  compiler does not verify the sample was generated from the same model.

### Proposed addition
A post-compilation verification pass: after `compiler.py` generates the `.xqasm` text,
assemble it with the verifier enabled (once the verifier exists) and surface any violations
as compiler errors with source-level context.

---

## 8. xqvm_py Reference Implementation

### Current state
`executor.py` performs no pre-execution validation. All errors are discovered during
`step()` calls.

### Proposed addition
A `BytecodeValidator` class in `xqvm_py/` that mirrors the Rust verifier semantics,
operating on the parsed instruction list before execution. This keeps the Python
implementation as a conformance oracle for the verifier itself (not just the executor).

---

## 9. Conformance Vectors Gap

Current conformance vectors in `conformance/vectors/` cover happy-path semantics and some
edge cases (e.g., `range_negative_skip`), but have limited coverage of error cases.

### Missing vector categories
- Unset register reads (expect `UnsetRegister` error).
- Type mismatch cases (e.g., LOAD on a Model register).
- Stack underflow and overflow cases.
- Mismatched loop nesting (NEXT without RANGE/ITER, vice versa).
- Invalid discrete k (XQMX with k=1).
- Invalid shift amounts.

Each category above should be covered by conformance vectors once a verifier is
implemented, to ensure Rust and Python verifiers agree on every error case.

---

## 10. Recommended Implementation Plan

### Phase 1 -- Structural integrity (low effort, high return)
1. Add bytecode header: magic `XQBC`, version `u8`, CRC32, input/output slot counts.
2. Validate loop nesting (RANGE/ITER vs. NEXT) at assembler and verifier level.
3. Pre-scan jump targets: verify every JUMP1/2 and JUMPI1/2 references a placed TARGET.
4. Validate constant operand constraints: `k >= 2` for XQMX/XSMX, shift amount 0-63.

### Phase 2 -- Register type verification (medium effort, critical)
5. Implement `xqvm::verifier` module: forward type-state pass over encoded bytecode.
6. Integrate into `Vm::new()` (or as a separate `Vm::verify()` call) so all callers benefit.
7. Mirror as `BytecodeValidator` in `xqvm_py/`.
8. Add error case conformance vectors for all `RegisterType` and `UnsetRegister` variants.

### Phase 3 -- Stack and data-flow analysis (medium effort, defence in depth)
9. Stack effect analyzer with per-instruction depth bounds.
10. Loop body net-effect check (depth must be neutral).
11. Reaching definitions analysis for unset-register detection across branches.
12. Post-compilation verifier pass in `xqcp`.

### Phase 4 -- Specification and governance
13. Add a "Bytecode Verification" section to `spec/xqvm/SPEC.md` defining which
    invariants the verifier must enforce.
14. Update `scripts/check-atomic-spec-mr.sh` to include `xqvm/src/verifier.rs` and
    `xqvm_py/validator.py` in the atomic-spec-MR layer check.

---

## Error Type Mapping

The following new error variants would be introduced in a dedicated `VerifierError` type
(separate from the runtime `Error` to preserve the existing diagnostic model):

```
VerifierError::RegisterTypeMismatch { offset, reg, expected, found }
VerifierError::UnsetRegisterRead    { offset, reg }
VerifierError::StackUnderflow       { offset }
VerifierError::StackOverflowRisk    { offset, depth }
VerifierError::LoopStackUnderflow   { offset }
VerifierError::UnmatchedLoop        { offset }
VerifierError::UndefinedJumpTarget  { offset, label }
VerifierError::InvalidOperandValue  { offset, field, value, constraint }
VerifierError::MagicMismatch        { found }
VerifierError::VersionMismatch      { found, expected }
VerifierError::ChecksumMismatch     { stored, computed }
```

All variants carry a byte offset so they can use the existing
`into_diagnostic(&program, source_name)` infrastructure for miette source annotation.

---

## Files to Touch (All Four Atomic Layers)

Per the atomic-spec-MR rule, any implementation work must span all four layers:

| Layer | Files |
|---|---|
| Spec | `spec/xqvm/SPEC.md` (new "Verification" section), `spec/xqvm/ISA.md` (stack effects table) |
| Rust VM | `xqvm/src/verifier.rs` (new), `xqvm/src/lib.rs` (re-export), `xqvm/src/error.rs` (VerifierError), `xqasm/src/assembler.rs` (integrate checks) |
| Python VM | `xqvm_py/validator.py` (new), `xqvm_py/executor.py` (optional pre-run hook), `xqvm_py/errors.py` (VerifierError class) |
| Conformance | `conformance/vectors/verification/` (new directory, error-case vectors) |
