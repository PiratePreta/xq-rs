# XQVM Bytecode Verifier

A bytecode verifier performs static analysis over a program before execution.
It catches structural and semantic errors without running the program, enabling
early rejection at load time or at the blockchain submission boundary.

The Rust implementation is `xqvm::verifier` (exposed via `xqffi.verifier` and
`xquad.verifier` Python bindings). There is no separate Python reference
verifier -- the Python reference VM (`xqvm_py`) defers to the Rust
implementation via FFI.

---

## Architecture

Verification is structured as composable [`Phase`] implementations. Each phase
performs one focused pass over a [`Program`] and maps failures to a
[`VerifierError`]. A [`Verifier`] runs phases sequentially, stopping at the
first failure.

The `scan` function is the shared kernel for Phase 1: it makes one linear pass
over the instruction bytes, builds the jump table, and returns the first
structural, jump-target, or loop-nesting violation. `Program::new` calls it to
populate the jump table (ignoring any error); `Verifier::default` calls it once
per invocation rather than three separate stream passes.

### Default pipeline (`Verifier::default`)

| Order | Phase | Struct | What it checks |
|-------|-------|--------|----------------|
| 1a | Structural | `StructuralPhase` | Truncated bytes, unknown opcodes |
| 1b | Jump target | `JumpTargetPhase` | Jump label >= target count |
| 1c | Loop nesting | `LoopNestingPhase` | Loop open/close balance, loop-context reads |
| 2 | Register type-state | `RegisterTypePhase` | Read-before-write, register type mismatches |
| 3 | Stack depth | `StackEffectPhase` | Stack underflow/overflow risk, loop body net-effect |

Phases 1a--1c share the `scan` kernel and run as one combined pass. Phases 2
and 3 are separate linear passes that follow.

Custom pipelines are supported via `Verifier::new().with_phase(p)`.

---

## Phase 1 -- Structural, Jump-Target, Loop-Nesting

### 1a: Structural

Every byte in the instruction stream must decode to a known opcode with the
correct number of operand bytes. Unrecognised opcode bytes and truncated operand
sequences are rejected immediately.

Errors: `TruncatedInstruction`, `BadOpcode`.

### 1b: Jump target

Every `JUMP1`, `JUMPI1`, `JUMP2`, and `JUMPI2` instruction must reference a
label id that is present in the jump table (i.e., a `TARGET` with that
sequential id exists in the program). Label ids are assigned sequentially in
program order during the `TARGET` pre-scan; any label id >= target count is
invalid.

Error: `UndefinedJumpTarget`.

### 1c: Loop nesting

`RANGE` and `ITER` instructions each open a loop frame; `NEXT` closes one. The
verifier tracks nesting depth and rejects:

- `NEXT`, `LVAL`, or `LIDX` reached with no open loop frame.
- Any loop frame still open at the end of the program, reported at the byte
  offset of the outermost unmatched opener.

Errors: `NoActiveLoop`, `UnmatchedLoop`.

---

## Phase 2 -- Register Type-State

A forward linear pass over `[RegType; 256]` (one slot per register). Each
register starts as `Unset`. Instructions that write a register advance its slot
to `Int`, `Vec`, `VecInt`, `VecXqmx`, `Model`, or `Sample`. Instructions that
read a register are checked against a `RegTypeReq` requirement.

Preconditions enforced:

- Reading an `Unset` register is rejected.
- Type mismatches (e.g., reading a `Vec` register as `Int`) are rejected.
- The `DROP` instruction resets a register to `Unset` in the type-state model -- a
  subsequent read is rejected as `ReadUnsetRegister`. Note: at VM runtime `DROP`
  writes `Int(0)` (a readable value), but the verifier applies the more conservative
  semantics to prevent accidental reads of dropped registers.

Errors: `ReadUnsetRegister`, `RegisterTypeMismatch`.

---

## Phase 3 -- Stack Depth and Loop Body Net-Effect

A forward linear pass maintains a running `depth: usize` (starting at 0) and a
loop stack of `(opener_offset, entry_depth)` pairs.

For each instruction, the delta from `Instruction::stack_effect()` is applied:

- `StackEffect::Delta(d)`: if `depth + d` would underflow, emit
  `StackUnderflow`. If the new depth exceeds 8192, emit `StackOverflowRisk`.
- `StackEffect::Reset` (`SCLR`): unconditionally sets `depth = 0`. **Note:**
  `SCLR` inside a loop body resets depth to 0 regardless of entry depth. If
  entry depth was N > 0, exit depth is 0 != N, which is reported as
  `LoopStackImbalance`. The message says "loop has non-zero stack effect" -- the
  root cause is a global stack reset mid-iteration, not a conventional push/pop
  imbalance.

Loop tracking (`RANGE`/`ITER`/`NEXT`) is handled after applying the depth
delta: on `RANGE`/`ITER`, record `(offset, depth)` on the loop stack; on
`NEXT`, pop the entry and compare with the current depth.

Errors: `StackUnderflow`, `StackOverflowRisk`, `LoopStackImbalance`.

### Limitation -- linear scan, no CFG

The scan is linear; conditional branches (`JUMPI1`/`JUMPI2`) are not followed.
This means:

- Underflow on a branch not taken may be missed (false negative).
- No false positives are produced on valid programs.

CFG-based data-flow analysis is tracked in QUI-513.

---

## Error Variants

| Variant | Phase | Description |
|---------|-------|-------------|
| `TruncatedInstruction` | 1a | Instruction stream ends mid-operand |
| `BadOpcode` | 1a | Opcode byte does not map to a known instruction |
| `UndefinedJumpTarget` | 1b | Jump references label id >= target count |
| `NoActiveLoop` | 1c | `NEXT`, `LVAL`, or `LIDX` with no open loop frame |
| `UnmatchedLoop` | 1c | Loop frame still open at end of program |
| `ReadUnsetRegister` | 2 | Register read before any write |
| `RegisterTypeMismatch` | 2 | Register holds wrong type for the instruction |
| `StackUnderflow` | 3 | `depth + delta` would go negative |
| `StackOverflowRisk` | 3 | `depth` would exceed 8192 |
| `LoopStackImbalance` | 3 | Loop body entry depth != exit depth |

---

## Per-Opcode Stack Effects

The `stack_delta` column of the `opcodes!` x-macro is the single source of
truth. The table below is derived from it. `Reset` means
`StackEffect::Reset` -- depth is set to 0 unconditionally (not a delta).

| Mnemonic | Stack delta | Notes |
|----------|-------------|-------|
| `TARGET` | 0 | |
| `JUMP1` | 0 | |
| `JUMPI1` | -1 | Pops condition |
| `JUMP2` | 0 | |
| `JUMPI2` | -1 | Pops condition |
| `LIDX` | 0 | |
| `LVAL` | 0 | |
| `NEXT` | 0 | |
| `RANGE` | -2 | Pops start, count |
| `ITER` | -2 | Pops start_idx, end_idx |
| `LOAD` | +1 | |
| `STOW` | -1 | |
| `DROP` | 0 | |
| `INPUT` | -1 | Pops slot index |
| `OUTPUT` | -1 | Pops slot index |
| `POP` | -1 | |
| `PUSH1`..`PUSH8` | +1 | |
| `SCLR` | Reset | depth -> 0 |
| `SWAP` | 0 | |
| `COPY` | +1 | |
| `ADD` | -1 | |
| `SUB` | -1 | |
| `MUL` | -1 | |
| `DIV` | -1 | |
| `MOD` | -1 | |
| `SQR` | 0 | |
| `ABS` | 0 | |
| `NEG` | 0 | |
| `MIN` | -1 | |
| `MAX` | -1 | |
| `INC` | 0 | |
| `DEC` | 0 | |
| `BITLEN` | 0 | |
| `EQ` | -1 | |
| `LT` | -1 | |
| `GT` | -1 | |
| `LTE` | -1 | |
| `GTE` | -1 | |
| `NOT` | 0 | |
| `AND` | -1 | |
| `OR` | -1 | |
| `XOR` | -1 | |
| `BAND` | -1 | |
| `BOR` | -1 | |
| `BXOR` | -1 | |
| `BNOT` | 0 | |
| `SHL` | -1 | |
| `SHR` | -1 | |
| `BQMX` | -1 | Pops size |
| `SQMX` | -1 | Pops size |
| `XQMX` | -2 | Pops size, k |
| `BSMX` | -1 | Pops size |
| `SSMX` | -1 | Pops size |
| `XSMX` | -2 | Pops size, k |
| `VEC` | 0 | |
| `VECI` | 0 | |
| `VECX` | 0 | |
| `VECPUSH` | -1 | |
| `VECGET` | 0 | Pops index, pushes element (net 0) |
| `VECSET` | -2 | Pops value, index |
| `VECLEN` | +1 | |
| `SLACK` | -2 | Pops capacity, start_index |
| `IDXGRID` | -2 | Pops cols, col, row; pushes index (net -2) |
| `IDXTRIU` | -1 | Pops j, i; pushes index (net -1) |
| `GETLINE` | 0 | Pops i, pushes value (net 0) |
| `SETLINE` | -2 | Pops value, i |
| `ADDLINE` | -2 | Pops delta, i |
| `GETQUAD` | -1 | Pops j, i; pushes value (net -1) |
| `SETQUAD` | -3 | Pops value, j, i |
| `ADDQUAD` | -3 | Pops delta, j, i |
| `RESIZE` | -2 | Pops cols, rows |
| `ROWFIND` | -1 | Pops value, row; pushes col (net -1) |
| `COLFIND` | -1 | Pops value, col; pushes row (net -1) |
| `ROWSUM` | 0 | Pops row, pushes sum (net 0) |
| `COLSUM` | 0 | Pops col, pushes sum (net 0) |
| `ONEHOTR` | -2 | Pops penalty, row |
| `ONEHOTC` | -2 | Pops penalty, col |
| `EXCLUDE` | -3 | Pops penalty, j, i |
| `IMPLIES` | -3 | Pops penalty, j, i |
| `EQUALITY` | -2 | Pops penalty, target |
| `ATLEAST` | -2 | Pops penalty, k |
| `ATLEASTW` | -2 | Pops penalty, k |
| `REDUCE` | -2 | Pops P_aux, var_b, var_a; pushes aux index (net -2) |
| `ENERGY` | +1 | Reads model and sample registers; pushes energy |
| `NOP` | 0 | |
| `HALT` | 0 | |