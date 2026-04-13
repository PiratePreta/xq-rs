# XQVM Bytecode Semantics

Authoritative reference for every instruction in the XQVM bytecode format.
Derived directly from the `opcodes!` x-macro in
`crates/bytecode/src/types/table.rs`, with mechanical behaviour verified
against `crates/vm/src/vm.rs`. When these two sources disagree, this document
reflects the VM implementation.

---

## VM State

A running VM holds four pieces of mutable state:

| Component | Type | Description |
|-----------|------|-------------|
| **Stack** | `Vec<i64>` | Operand stack. Max depth: **8 192** items. Overflow → error. |
| **Register file** | `[RegVal; 256]` | 256 slots, indexed `r0`–`r255`. Initialised to `Int(0)`. |
| **Loop stack** | `Vec<LoopFrame>` | Frames pushed by `RANGE`/`ITER`, popped by `NEXT`. |
| **Calldata / Outputs** | `Vec<RegVal>` | Read-only input slots (`INPUT`) and writable output slots (`OUTPUT`). |

### `RegVal` — the register value type

Each register holds one variant:

| Variant | Rust type | Notes |
|---------|-----------|-------|
| `Int(i64)` | `i64` | Default value for every register. |
| `VecInt(Vec<i64>)` | `Vec<i64>` | Integer vector. |
| `VecXqmx(Vec<XqmxModel>)` | `Vec<XqmxModel>` | Vector of models. |
| `Model(XqmxModel)` | struct | QUBO/Ising/discrete Hamiltonian. |
| `Sample(XqmxSample)` | struct | Variable-assignment vector. |

---

## Notation

- **Stack diagrams** — `[..., a, b] → [..., r]`, rightmost = **top**. `b` is
  popped first.
- **`reg`** — the `u8` operand encoded in the instruction byte stream.
- Assignments use `←` (register write) and `→` (stack push).
- **Wrapping** — all integer arithmetic uses wrapping semantics on `i64`
  (no panic on overflow; result truncated to 64 bits).
- **Reserved** — opcodes `0x08`, `0x0D`, `0x19`, `0x35` are unassigned gaps;
  the decoder rejects them as illegal.
- **Register effect** column uses three access modes:
  - **`read`** — register contents are inspected but not changed.
  - **`write`** — register is replaced wholesale with a new value.
  - **`mutate`** — register's existing value is modified in-place (e.g.
    appending to a vec, incrementing a coefficient).

---

## Control Flow

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x00` | `NOP` | — | `[...] → [...]` | — | No operation. |
| `0x01` | `TARGET` | — | `[...] → [...]` | — | Mark a valid jump destination. Required at every label that `JUMP`/`JUMPI` may target; treated as `NOP` at runtime. |
| `0x02` | `JUMP` | `label: u16` | `[...] → [...]` | — | Seek the instruction stream to `jump_table[label].start`. Unconditional. |
| `0x03` | `JUMPI` | `label: u16` | `[..., cond] → [...]` | — | Pop `cond`. If `cond != 0`, seek to `jump_table[label].start`; otherwise fall through. |
| `0x04` | `NEXT` | — | `[...] → [...]` | — | Advance the active loop frame. For `Range`: increment `current`; if `current < end`, seek to `body_start`, else pop frame and fall through. For `Iter`: increment `index`; if `index < len(reg)`, seek to `body_start`, else pop frame and fall through. Errors if no loop frame is active. |
| `0x05` | `LVAL` | `reg: Register` | `[...] → [...]` | `write` — `reg ← Int(current)` for Range; `reg ← VecInt[index]` or `reg ← VecXqmx[index]` for Iter | Copy the current loop value into `reg`. For `Range`: `reg ← Int(current)`. For `Iter`: `reg ← vec[index]` (element type preserved: `Int` or `Model`). |
| `0x06` | `RANGE` | — | `[..., start, count] → [...]` | — | Pop `count`, then `start`. Push a `Range { current: start, end: start.wrapping_add(count) }` loop frame; the next instruction's byte offset becomes `body_start`. |
| `0x07` | `ITER` | `reg: Register` | `[...] → [...]` | `read` — validates `reg` holds `VecInt` or `VecXqmx` | Validate that `reg` holds a `VecInt` or `VecXqmx`; push an `Iter { reg, index: 0 }` loop frame. The next instruction's byte offset becomes `body_start`. |
| `0x08` | `LIDX` | `reg: Register` | `[...] → [...]` | `write` — `reg ← Int(current)` for Range; `reg ← Int(index)` for Iter | Copy the current loop *index* into `reg` as `Int`. For `Range` the values are themselves indices, so this is identical to `LVAL`. For `Iter` it returns the position of the current element within the iterated vec. Errors with `NoActiveLoop` if no loop frame is active. |
| `0x09` | `HALT` | — | `[...] → [...]` | — | Stop execution immediately. |

---

## Register I/O

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x0A` | `LOAD` | `reg: Register` | `[...] → [..., v]` | `read` — `reg` must hold `Int(v)` | `reg` must hold `Int(v)`. Push `v`. Errors if `reg` holds any other variant. |
| `0x0B` | `STOW` | `reg: Register` | `[..., v] → [...]` | `write` — `reg ← Int(v)` | Pop `v`. Write `reg ← Int(v)`. |
| `0x0C` | `DROP` | `reg: Register` | `[...] → [...]` | `write` — `reg ← Int(0)` | Write `reg ← Int(0)`, releasing any heap allocation the slot held. |
| `0x0E` | `INPUT` | `reg: Register` | `[..., s] → [...]` | `write` — `reg ← calldata[s]` (any `RegVal`) | Pop `s` (slot index). `s` must be a valid index into calldata (`0 ≤ s < len`). Clone `calldata[s]` into `reg`. Any `RegVal` variant is transferable. |
| `0x0F` | `OUTPUT` | `reg: Register` | `[..., s] → [...]` | `read` — `reg` cloned to `outputs[s]` | Pop `s` (slot index). `s` must be a valid index into the output array (`0 ≤ s < len`). Clone `reg`'s value into `outputs[s]`. |

---

## Stack Manipulation

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x10` | `POP` | — | `[..., a] → [...]` | — | Discard the top of the stack. |
| `0x11` | `PUSH1` | `val: [u8; 1]` | `[...] → [..., v]` | — | Interpret `val` as a 1-byte big-endian signed integer, sign-extend to `i64`, push `v`. |
| `0x12` | `PUSH2` | `val: [u8; 2]` | `[...] → [..., v]` | — | Same, 2 bytes. |
| `0x13` | `PUSH3` | `val: [u8; 3]` | `[...] → [..., v]` | — | Same, 3 bytes. |
| `0x14` | `PUSH4` | `val: [u8; 4]` | `[...] → [..., v]` | — | Same, 4 bytes. |
| `0x15` | `PUSH5` | `val: [u8; 5]` | `[...] → [..., v]` | — | Same, 5 bytes. |
| `0x16` | `PUSH6` | `val: [u8; 6]` | `[...] → [..., v]` | — | Same, 6 bytes. |
| `0x17` | `PUSH7` | `val: [u8; 7]` | `[...] → [..., v]` | — | Same, 7 bytes. |
| `0x18` | `PUSH8` | `val: [u8; 8]` | `[...] → [..., v]` | — | Interpret `val` as a full 8-byte big-endian `i64`, push `v`. |
| `0x1A` | `SCLR` | — | `[...] → []` | — | Clear the entire value stack. |
| `0x1B` | `SWAP` | — | `[..., a, b] → [..., b, a]` | — | Swap the top two elements. Errors if stack depth < 2. |
| `0x1C` | `COPY` | — | `[..., a] → [..., a, a]` | — | Duplicate the top of the stack without consuming it. |

---

## Arithmetic

All operations are on `i64` with **wrapping** semantics (no overflow trap).

| Code | Mnemonic | Stack effect | Register effect | Interpretation |
|------|----------|--------------|-----------------|----------------|
| `0x20` | `ADD` | `[..., a, b] → [..., a+b]` | — | Wrapping addition. |
| `0x21` | `SUB` | `[..., a, b] → [..., a-b]` | — | Wrapping subtraction. |
| `0x22` | `MUL` | `[..., a, b] → [..., a*b]` | — | Wrapping multiplication. |
| `0x23` | `DIV` | `[..., a, b] → [..., a/b]` | — | Truncating integer division. Errors if `b == 0`. |
| `0x24` | `MOD` | `[..., a, b] → [..., a%b]` | — | Truncating remainder. Errors if `b == 0`. |
| `0x25` | `SQR` | `[..., a] → [..., a*a]` | — | Wrapping square. |
| `0x26` | `ABS` | `[..., a] → [..., \|a\|]` | — | Wrapping absolute value (`i64::MIN.abs()` wraps to `i64::MIN`). |
| `0x27` | `NEG` | `[..., a] → [..., -a]` | — | Wrapping negation. |
| `0x28` | `MIN` | `[..., a, b] → [..., min(a,b)]` | — | Signed minimum. |
| `0x29` | `MAX` | `[..., a, b] → [..., max(a,b)]` | — | Signed maximum. |
| `0x2A` | `INC` | `[..., a] → [..., a+1]` | — | Wrapping increment. |
| `0x2B` | `DEC` | `[..., a] → [..., a-1]` | — | Wrapping decrement. |

---

## Comparison

Results are `1i64` (true) or `0i64` (false). All comparisons are signed.

| Code | Mnemonic | Stack effect | Register effect | Interpretation |
|------|----------|--------------|-----------------|----------------|
| `0x30` | `EQ` | `[..., a, b] → [..., a==b ? 1 : 0]` | — | Signed equality. |
| `0x31` | `LT` | `[..., a, b] → [..., a<b ? 1 : 0]` | — | Signed less-than. |
| `0x32` | `GT` | `[..., a, b] → [..., a>b ? 1 : 0]` | — | Signed greater-than. |
| `0x33` | `LTE` | `[..., a, b] → [..., a<=b ? 1 : 0]` | — | Signed less-or-equal. |
| `0x34` | `GTE` | `[..., a, b] → [..., a>=b ? 1 : 0]` | — | Signed greater-or-equal. |

---

## Logical Boolean

Operands are treated as booleans: `0` is false, any non-zero value is true.
Results are `1i64` or `0i64`.

| Code | Mnemonic | Stack effect | Register effect | Interpretation |
|------|----------|--------------|-----------------|----------------|
| `0x36` | `NOT` | `[..., a] → [..., a==0 ? 1 : 0]` | — | Logical NOT. |
| `0x37` | `AND` | `[..., a, b] → [..., (a!=0 && b!=0) ? 1 : 0]` | — | Logical AND (short-circuit not applicable; both operands already popped). |
| `0x38` | `OR` | `[..., a, b] → [..., (a!=0 \|\| b!=0) ? 1 : 0]` | — | Logical OR. |
| `0x39` | `XOR` | `[..., a, b] → [..., ((a!=0) ^ (b!=0)) ? 1 : 0]` | — | Logical XOR. True iff exactly one operand is non-zero. |

---

## Bitwise

Operate on raw `i64` bit patterns.

| Code | Mnemonic | Stack effect | Register effect | Interpretation |
|------|----------|--------------|-----------------|----------------|
| `0x3A` | `BAND` | `[..., a, b] → [..., a & b]` | — | Bitwise AND. |
| `0x3B` | `BOR` | `[..., a, b] → [..., a \| b]` | — | Bitwise OR. |
| `0x3C` | `BXOR` | `[..., a, b] → [..., a ^ b]` | — | Bitwise XOR. |
| `0x3D` | `BNOT` | `[..., a] → [..., ~a]` | — | Bitwise NOT (one's complement). |
| `0x3E` | `SHL` | `[..., a, b] → [..., a << b]` | — | Left shift. `b` must satisfy `0 ≤ b < 64`; otherwise errors. Signed `i64` left shift, wrapping behaviour on overflow. |
| `0x3F` | `SHR` | `[..., a, b] → [..., a >> b]` | — | Arithmetic (sign-preserving) right shift on `i64`. `b` must satisfy `0 ≤ b < 64`. The sign bit is replicated, matching `XQVM_SPEC.md` and Rust's native `i64 >> b` operator. |

---

## Allocators

These instructions allocate quantum/combinatorial objects into registers.
An invalid (negative) size pops as-is; a non-representable `usize` silently
uses size `0`.

### Model Allocators

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x40` | `BQMX` | `reg: Register` | `[..., size] → [...]` | `write` — `reg ← Model(domain: Binary, size)` | Pop `size`. `reg ← Model(XqmxModel { domain: Binary, size, linear: {}, quad: {} })`. Variable domain: `{0, 1}`. |
| `0x41` | `SQMX` | `reg: Register` | `[..., size] → [...]` | `write` — `reg ← Model(domain: Spin, size)` | Pop `size`. `reg ← Model(XqmxModel { domain: Spin, size, ... })`. Variable domain: `{-1, 1}`. |
| `0x42` | `XQMX` | `reg: Register` | `[..., size, k] → [...]` | `write` — `reg ← Model(domain: Discrete(k), size)` | Pop `k`, then `size`. `reg ← Model(XqmxModel { domain: Discrete(k), size, ... })`. Variable domain: `{-k, -(k-1), …, k-2, k-1}` (signed, centered, chromatic). Errors with `InvalidDiscreteK` when `k < 2`. |

### Sample Allocators

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x43` | `BSMX` | `reg: Register` | `[..., size] → [...]` | `write` — `reg ← Sample(domain: Binary, values: [0; size])` | Pop `size`. `reg ← Sample(XqmxSample { domain: Binary, values: vec![0; size] })`. |
| `0x44` | `SSMX` | `reg: Register` | `[..., size] → [...]` | `write` — `reg ← Sample(domain: Spin, values: [-1; size])` | Pop `size`. `reg ← Sample(XqmxSample { domain: Spin, values: vec![-1; size] })`. Default value is `-1` (spin-down). |
| `0x45` | `XSMX` | `reg: Register` | `[..., size, k] → [...]` | `write` — `reg ← Sample(domain: Discrete(k), values: [0; size])` | Pop `k`, then `size`. `reg ← Sample(XqmxSample { domain: Discrete(k), values: vec![0; size] })`. Variable domain: `{-k, -(k-1), …, k-2, k-1}` (signed, centered). Errors with `InvalidDiscreteK` when `k < 2`. The default value `0` is always in-domain. |

### Vec Allocators

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x4A` | `VEC` | `reg: Register` | `[...] → [...]` | `write` — `reg ← VecInt([])` | `reg ← VecInt([])`. Identical to `VECI` at runtime; documented as "untyped" but always creates an integer vec. |
| `0x4B` | `VECI` | `reg: Register` | `[...] → [...]` | `write` — `reg ← VecInt([])` | `reg ← VecInt([])`. Explicit integer vec. |
| `0x4C` | `VECX` | `reg: Register` | `[...] → [...]` | `write` — `reg ← VecXqmx([])` | `reg ← VecXqmx([])`. Vec of `XqmxModel` values. |

---

## Vector Access

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x50` | `VECPUSH` | `reg: Register` | `[..., v] → [...]` | `mutate` — appends `v` to `reg`'s `VecInt` | Pop `v`. `reg` must hold `VecInt`. Append `v`. |
| `0x51` | `VECGET` | `reg: Register` | `[..., idx] → [..., v]` | `read` — `reg` must hold `VecInt` | Pop `idx`. `reg` must hold `VecInt`. Bounds-check: `0 ≤ idx < len`. Push `vec[idx]`. |
| `0x52` | `VECSET` | `reg: Register` | `[..., idx, v] → [...]` | `mutate` — sets `reg.vec[idx] ← v` | Pop `v` (value), then `idx` (index). `reg` must hold `VecInt`. Bounds-check: `0 ≤ idx < len`. Set `vec[idx] ← v`. |
| `0x53` | `VECLEN` | `reg: Register` | `[...] → [..., n]` | `read` — `reg` must hold `VecInt` or `VecXqmx` | `reg` must hold `VecInt` or `VecXqmx`. Push `len(reg)` as `i64`. |

---

## Index Math

Utilities for mapping 2-D coordinates to flat array indices. All arithmetic
is wrapping on `i64`.

| Code | Mnemonic | Stack effect | Register effect | Interpretation |
|------|----------|--------------|-----------------|----------------|
| `0x5A` | `IDXGRID` | `[..., row, col, cols] → [..., row*cols+col]` | — | Row-major flat index. Pops `cols`, then `col`, then `row`; pushes `row.wrapping_mul(cols).wrapping_add(col)`. |
| `0x5B` | `IDXTRIU` | `[..., i, j] → [..., j*(j-1)/2+i]` | — | Upper-triangular index for the pair `(i, j)` with `i ≤ j`. Pops `j`, then `i`; pushes `j.wrapping_mul(j.wrapping_sub(1)) / 2 + i`. |

---

## XQMX Coefficient Access

Read and write the linear (bias) and quadratic (coupling) coefficients of a
`Model` register. Missing entries read as `0`; writes create the entry on the
first call. All coefficient values are `i64`; `reg` must hold `Model`.

### Linear Coefficients

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x60` | `GETLINE` | `reg: Register` | `[..., i] → [..., linear[i]]` | `read` — `reg.model.linear[i]` | Pop `i`. Push `model.linear[i]` (0 if absent). |
| `0x61` | `SETLINE` | `reg: Register` | `[..., i, v] → [...]` | `mutate` — `reg.model.linear[i] ← v` | Pop `v`, then `i`. Set `model.linear[i] ← v`. |
| `0x62` | `ADDLINE` | `reg: Register` | `[..., i, δ] → [...]` | `mutate` — `reg.model.linear[i] += δ` | Pop `δ`, then `i`. `model.linear[i] += δ`. |

### Quadratic Coefficients

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x63` | `GETQUAD` | `reg: Register` | `[..., i, j] → [..., quad[i,j]]` | `read` — `reg.model.quad[i,j]` | Pop `j`, then `i`. Push `model.quad[i,j]` (0 if absent). |
| `0x64` | `SETQUAD` | `reg: Register` | `[..., i, j, v] → [...]` | `mutate` — `reg.model.quad[i,j] ← v` | Pop `v`, then `j`, then `i`. Set `model.quad[i,j] ← v`. |
| `0x65` | `ADDQUAD` | `reg: Register` | `[..., i, j, δ] → [...]` | `mutate` — `reg.model.quad[i,j] += δ` | Pop `δ`, then `j`, then `i`. `model.quad[i,j] += δ`. |

---

## XQMX Grid

A model can optionally be given 2-D grid dimensions so that variables are
addressed as `(row, col)` with flat index `row * cols + col`. `reg` must hold `Model`.

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x66` | `RESIZE` | `reg: Register` | `[..., rows, cols] → [...]` | `mutate` — `reg.model.rows ← rows; reg.model.cols ← cols` | Pop `cols`, then `rows`. Set `model.rows ← rows`, `model.cols ← cols`. Both must be `> 0`; otherwise errors with `InvalidGridDimensions`. |
| `0x67` | `ROWFIND` | `reg: Register` | `[..., row, value] → [..., col]` | `read` — scans `reg.model.linear[row*cols..]` | Pop `value`, then `row`. Scan `linear[row*cols]..linear[row*cols + cols)` for the first entry equal to `value`. Push the column index, or `-1` if not found. |
| `0x68` | `COLFIND` | `reg: Register` | `[..., col, value] → [..., row]` | `read` — scans `reg.model.linear[r*cols+col]` for each row | Pop `value`, then `col`. Scan `linear[0*cols+col], linear[1*cols+col], …` across all rows. Push the row index of the first match, or `-1` if not found. |
| `0x69` | `ROWSUM` | `reg: Register` | `[..., row] → [..., sum]` | `read` — sums `reg.model.linear[row*cols..row*cols+cols]` | Pop `row`. Push `Σ linear[row*cols + c]` for `c` in `0..cols`. |
| `0x6A` | `COLSUM` | `reg: Register` | `[..., col] → [..., sum]` | `read` — sums `reg.model.linear[r*cols+col]` for each row | Pop `col`. Push `Σ linear[r*cols + col]` for `r` in `0..rows`. |

---

## XQMX High-Level Constraints

These instructions inject QUBO penalty terms for common combinatorial
constraints, expanding into linear and quadratic coefficient deltas
automatically. `reg` must hold `Model` with grid dimensions pre-set by
`RESIZE`. All coefficients are `i64`.

### `0x70` — `ONEHOTR reg`

**Stack:** `[..., row, penalty] → [...]`
**Register effect:** `mutate` — adds to `reg.model.linear` and `reg.model.quad` for all variables in `row`

Pop `penalty`, then `row`. Apply the one-hot constraint over all variables in
grid row `row`:

```
H += penalty * (Σ x_{row,c} - 1)²
   = penalty * (Σ x_{row,c}² - 2·Σ x_{row,c} + 1)
```

Expanding (binary variables: `x² = x`):

```
linear[row*cols + c]            += -penalty          for each c in 0..cols
quad[row*cols + ci, row*cols + cj]  += 2*penalty     for each pair ci < cj
```

### `0x71` — `ONEHOTC reg`

**Stack:** `[..., col, penalty] → [...]`
**Register effect:** `mutate` — adds to `reg.model.linear` and `reg.model.quad` for all variables in `col`

Pop `penalty`, then `col`. One-hot over all variables in grid column `col`:

```
linear[ri*cols + col]            += -penalty          for each ri in 0..rows
quad[ri*cols + col, rj*cols + col]  += 2*penalty     for each pair ri < rj
```

### `0x72` — `EXCLUDE reg`

**Stack:** `[..., i, j, penalty] → [...]`
**Register effect:** `mutate` — `reg.model.quad[i,j] += penalty`

Pop `penalty`, then `j`, then `i`. Add mutual-exclusion: penalise
`x_i = 1` and `x_j = 1` simultaneously.

```
quad[i, j] += penalty
```

### `0x73` — `IMPLIES reg`

**Stack:** `[..., i, j, penalty] → [...]`
**Register effect:** `mutate` — `reg.model.linear[i] += penalty; reg.model.quad[i,j] += -penalty`

Pop `penalty`, then `j`, then `i`. Add implication `i → j`: penalise
`x_i = 1` with `x_j = 0`.

```
H += penalty * x_i * (1 - x_j) = penalty*x_i - penalty*x_i*x_j

linear[i]    += penalty
quad[i, j]   += -penalty
```

---

## Energy Evaluation

### `0x7F` — `ENERGY model sample`

**Stack:** `[...] → [..., E]`
**Register effect:** `read` — `model` and `sample` are both read-only

The `model` register must hold a `Model`, and the `sample` register must hold
a `Sample` -- xq-rs no longer accepts a `Model` in the sample slot (the
"model-as-sample" shortcut was removed in QUI-410 to align with `XQVM_SPEC.md`
and the xq-py reference). Construct the sample explicitly via `BSMX` / `SSMX`
/ `XSMX`, or pass an `XqmxSample` through calldata. A `RegisterType` error is
raised if either register holds the wrong kind of value.

Evaluate the Hamiltonian:

```
E = Σᵢ linear[i] * x[i]  +  Σ_{i<j} quad[i,j] * x[i] * x[j]
```

Push the result as `i64`. Errors with `SizeMismatch` if `len(sample) != model.size`.

`ENERGY` is the only instruction with two register operands.

---

## Runtime Limits

| Limit | Default | Notes |
|-------|---------|-------|
| Stack depth | 8 192 items | `StackOverflow` error if exceeded. |
| Step count | 10 000 000 | `StepLimitExceeded` error if exceeded. Configurable via `Vm::set_step_limit`. Passing `0` sets the limit to `u64::MAX`. |

---

## Reserved / Illegal Opcodes

The following byte values are explicitly unassigned gaps; the decoder rejects
them as illegal opcodes:

`0x08`, `0x0D`, `0x19`, `0x35`

All other byte values outside the assigned ranges are likewise illegal.
