# XQVM Technical Specification

## X-Quadratic Virtual Machine

A specialized virtual machine for encoding, verifying, and decoding quantum optimization problems. Provides a unified instruction set for manipulating quadratic models across variable domains (binary, spin, chromatic).

## Three-Program Architecture

Each optimization problem is defined by three independent programs sharing the same instruction set:

- **Encoder** — Transforms problem-specific input into an XQMX model suitable for quantum optimization.
- **Verifier** — Validates solution quality and constraint satisfaction.
- **Decoder** — Transforms quantum solution back into problem-specific output.

Programs execute independently with no shared state. Communication occurs only through calldata (`input`) and results (`output`).

---

## Machine State

```python
{
    "pc": 0,                  # Program counter
    "stack": [],              # Integer-only stack

    "registers": {            # slot (0-255) → value
        0: 42,                # int
        1: [1, 2, 3, 4],      # vec
        2: {                  # xqmx
            "mode": "model",          # "model" | "sample"
            "domain": [0, 1],         # [0,1] | [-1,1] | [-k,...,k-1]
            "size": 9,
            "rows": None,
            "cols": None,
            "linear": {},             # sparse: var_index → value
            "quadratic": {}           # sparse: (i,j) → value, i < j
        }
    },

    "jc": {
        "targets": {},        # target_id → pc
        "loop_stack": []      # loop state stack (LIFO)
    },

    "input": {},              # calldata: slot → value
    "output": {}              # results: slot → value
}
```

---

## Type System

### Stack

- Integer only. All primitive operations work on integers.
- **Value type:** signed 64-bit integer. Valid range `[-2^63, 2^63 - 1]`.
- **Overflow:** any operation that would produce a value outside the i64 range raises `ArithmeticOverflow` (same class as `DivisionByZero`, `StackOverflow`). This applies to arithmetic, shift, and `INC`/`DEC`/`NEG`/`ABS`/`SQR` opcodes, as well as integer values entering the VM through `INPUT`. Implementations backed by fixed-width 64-bit integers may instead wrap silently — such wrapping is implementation-defined and programs that rely on it are non-portable.
- Maximum depth: 8192 (2^13)

### Registers (`r0`–`r255`)

- 8-bit slot ID (0-255)
- Types: `int` | `vec` | `xqmx`
- No pointers, no type coercion
- Only `int` registers exchange with stack via `LOAD`/`STOW`
- `vec` and `xqmx` accessed only through specialized opcodes

### `vec`

- Homogeneous dynamic array: `vec<int>`, `vec<xqmx>` with support for nesting (`vec<vec<int>>`)
- Element type inferred and locked on first push, or explicit via `VECI`/`VECX` opcodes. Type validation on mutate operations
- Tracks length and capacity

### `xqmx`

- Sparse x-quadratic matrix
- **Mode:** `model` (linear & quadratic are hamiltonian coefficients) or `sample` (linear are variable assignments, quadratic is nil)
- **Domain:** `[0,1]` binary, `[-1,1]` spin, `[-k, ..., k-1]` chromatic
- **Dimension:** `size` (total linear variables), optional `rows`/`cols` for grid layout
- **Storage:** Sparse tables for `linear` and `quadratic`
- Constraint opcodes (ONEHOTR, ONEHOTC, EXCLUDE, IMPLIES) are only valid in model mode
- ENERGY computes the Hamiltonian energy of a sample against a model

---

## Instruction Set (87 opcodes)

### Notation

The following conventions are used throughout this section to describe opcode behaviour. They are documentation shorthand only and do not prescribe implementation details.

- **Stack diagrams** — `[..., a, b] → [..., r]`, rightmost = **top**. `b` is popped first.
- **`reg`** — the register operand encoded in the instruction.
- **Register effect** column uses three access modes:
  - **`read`** — register contents are inspected but not changed.
  - **`write`** — register is replaced wholesale with a new value.
  - **`mutate`** — register's existing value is modified in-place (e.g. appending to a vec, incrementing a coefficient).
- Assignments use `←` (register write) and `→` (stack push).
- **Boolean representation** — `0` is false, any non-zero value is true. Boolean-producing opcodes always push `0` or `1`.

### Control Flow

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x00` | `TARGET` | — | `[...] → [...]` | — | Mark a valid jump destination. No operand in bytecode; during pre-scan, each TARGET is assigned a sequential ID (0, 1, 2, ...) in program order and registered at the current PC. No-op at runtime. In assembly source, `TARGET .N` is syntactic sugar — the `.N` label is resolved by the assembler and not encoded. |
| `0x01` | `JUMP1` | `.N` | `[...] → [...]` | — | Set PC to the instruction at `targets[N]`, where N is the sequential target ID (u8). Unconditional. Error: `TargetNotFound` if N is undefined. |
| `0x02` | `JUMPI1` | `.N` | `[..., cond] → [...]` | — | Pop `cond`. If `cond != 0`, set PC to `targets[N]` (u8); otherwise fall through. Error: `TargetNotFound` if N is undefined and condition is non-zero. |
| `0x03` | `JUMP2` | `hi`, `lo` | `[...] → [...]` | — | Set PC to the instruction at `targets[N]`, where N is the sequential target ID encoded as u16 big-endian (`N = hi << 8 \| lo`). Unconditional. Error: `TargetNotFound` if N is undefined. |
| `0x04` | `JUMPI2` | `hi`, `lo` | `[..., cond] → [...]` | — | Pop `cond`. If `cond != 0`, set PC to `targets[N]` (u16 big-endian); otherwise fall through. Error: `TargetNotFound` if N is undefined and condition is non-zero. |
| `0x05` | `LIDX` | `reg` | `[...] → [...]` | `write` — `reg ← index + start_offset` | Copy the current loop index (offset-adjusted) into `reg`. For RANGE loops: equivalent to LVAL (values are indices). For ITER loops: returns the original vec index (`frame.index + start_idx`). Error: `LoopError` if no active loop. |
| `0x06` | `LVAL` | `reg` | `[...] → [...]` | `write` — `reg ← values[index]` | Copy the current loop value into `reg`. For RANGE loops: `reg ← int`. For ITER loops: `reg ← vec element` (type preserved: int or xqmx). Error: `LoopError` if no active loop. |
| `0x07` | `NEXT` | — | `[...] → [...]` | — | Advance the active loop frame index. If more values remain, set PC to `frame.target` (loop body start). Otherwise pop the frame and fall through. Error: `LoopError` if no loop frame is active. |
| `0x08` | `RANGE` | — | `[..., start, count] → [...]` | — | Pop `count`, then `start`. Generate values `[start, start+1, ..., start+count-1]`. Push a loop frame with `target = PC+1` and `start_offset = start`. If `count <= 0`, the loop body is skipped entirely. |
| `0x09` | `ITER` | `reg` | `[..., start_idx, end_idx] → [...]` | `read` — validates `reg` holds `vec` | Pop `end_idx`, then `start_idx`. Read vec from `reg`. Copy elements `vec[start_idx:end_idx]` into a loop frame with `target = PC+1` and `start_offset = start_idx`. If `start_idx >= end_idx`, the loop body is skipped. Elements are copied for immutability. Error: `TypeMismatch` if `reg` is not a vec. |
| `0xF0` | `NOP` | — | `[...] → [...]` | — | No operation. |
| `0xFF` | `HALT` | — | `[...] → [...]` | — | Stop execution immediately. |

**JUMP sugar:** In assembly source, `JUMP .N` and `JUMPI .N` are syntactic sugar. The assembler resolves the label and selects `JUMP1`/`JUMPI1` (u8) or `JUMP2`/`JUMPI2` (u16) based on the target ID value, mirroring the `PUSH`/`PUSH1`–`PUSH8` sugar pattern.

---

### Register I/O

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x0A` | `LOAD` | `reg` | `[...] → [..., v]` | `read` — `reg` must hold `int` | Push the integer value from `reg` onto the stack. Error: `TypeMismatch` if `reg` holds vec or xqmx. Error: `RegisterNotFound` if `reg` is unset. |
| `0x0B` | `STOW` | `reg` | `[..., v] → [...]` | `write` — `reg ← int(v)` | Pop `v`. Write `reg ← v` as an integer. |
| `0x0C` | `DROP` | `reg` | `[...] → [...]` | `write` — `reg ← unset` | Clear the register, releasing any value it held. The register becomes unset. No error if already unset. |
| `0x0E` | `INPUT` | `reg` | `[..., s] → [...]` | `write` — `reg ← input[s]` | Pop `s` (slot index). Copy `input[s]` into `reg`. Any value type is transferable (int, vec, or xqmx). Returns `None` if slot is not set. |
| `0x0F` | `OUTPUT` | `reg` | `[..., s] → [...]` | `read` — `reg` value copied to `output[s]` | Pop `s` (slot index). Copy `reg`'s value into `output[s]`. Any value type is transferable. Error: `RegisterNotFound` if `reg` is unset. |

---

### Stack Manipulation

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x10` | `POP` | — | `[..., a] → [...]` | — | Discard the top of the stack. Error: `StackUnderflow` if empty. |
| `0x11` | `PUSH1` | `val: 1 × <int>` | `[...] → [..., v]` | — | Interpret `val` as a 1-byte big-endian signed two's complement integer, push `v`. |
| `0x12` | `PUSH2` | `val: 2 × <int>` | `[...] → [..., v]` | — | Same, 2 bytes. |
| `0x13` | `PUSH3` | `val: 3 × <int>` | `[...] → [..., v]` | — | Same, 3 bytes. |
| `0x14` | `PUSH4` | `val: 4 × <int>` | `[...] → [..., v]` | — | Same, 4 bytes. |
| `0x15` | `PUSH5` | `val: 5 × <int>` | `[...] → [..., v]` | — | Same, 5 bytes. |
| `0x16` | `PUSH6` | `val: 6 × <int>` | `[...] → [..., v]` | — | Same, 6 bytes. |
| `0x17` | `PUSH7` | `val: 7 × <int>` | `[...] → [..., v]` | — | Same, 7 bytes. |
| `0x18` | `PUSH8` | `val: 8 × <int>` | `[...] → [..., v]` | — | Same, 8 bytes (full-width signed integer). |
| `0x1A` | `SCLR` | — | `[...] → []` | — | Clear the entire value stack. Stack depth becomes 0. |
| `0x1B` | `SWAP` | — | `[..., a, b] → [..., b, a]` | — | Swap the top two elements. Error: `StackUnderflow` if depth < 2. |
| `0x1C` | `COPY` | — | `[..., a] → [..., a, a]` | — | Duplicate the top of the stack without consuming it. Error: `StackUnderflow` if empty. |

Operand bytes are concatenated in big-endian order (most significant byte first) and interpreted as a signed two's complement integer. For example, `PUSH2 0xFF 0xFE` pushes −2.

---

### Arithmetic

| Code | Mnemonic | Stack effect | Interpretation |
|------|----------|--------------|----------------|
| `0x20` | `ADD` | `[..., a, b] → [..., a+b]` | Addition. |
| `0x21` | `SUB` | `[..., a, b] → [..., a-b]` | Subtraction. |
| `0x22` | `MUL` | `[..., a, b] → [..., a*b]` | Multiplication. |
| `0x23` | `DIV` | `[..., a, b] → [..., a/b]` | Truncating integer division (rounds toward negative infinity). Error: `DivisionByZero` if `b == 0`. |
| `0x24` | `MOD` | `[..., a, b] → [..., a%b]` | Modulo (result has same sign as divisor). Error: `DivisionByZero` if `b == 0`. |
| `0x25` | `SQR` | `[..., a] → [..., a*a]` | Square. |
| `0x26` | `ABS` | `[..., a] → [..., \|a\|]` | Absolute value. |
| `0x27` | `NEG` | `[..., a] → [..., -a]` | Negation. |
| `0x28` | `MIN` | `[..., a, b] → [..., min(a,b)]` | Signed minimum. |
| `0x29` | `MAX` | `[..., a, b] → [..., max(a,b)]` | Signed maximum. |
| `0x2A` | `INC` | `[..., a] → [..., a+1]` | Increment. |
| `0x2B` | `DEC` | `[..., a] → [..., a-1]` | Decrement. |

Binary operations pop the second operand first: `PUSH a; PUSH b; SUB` → `a - b`. Top of stack is always the second operand.

---

### Comparison

Results are `1` (true) or `0` (false). All comparisons are signed.

| Code | Mnemonic | Stack effect | Interpretation |
|------|----------|--------------|----------------|
| `0x30` | `EQ` | `[..., a, b] → [..., a==b ? 1 : 0]` | Equality. |
| `0x31` | `LT` | `[..., a, b] → [..., a<b ? 1 : 0]` | Less-than. |
| `0x32` | `GT` | `[..., a, b] → [..., a>b ? 1 : 0]` | Greater-than. |
| `0x33` | `LTE` | `[..., a, b] → [..., a<=b ? 1 : 0]` | Less-or-equal. |
| `0x34` | `GTE` | `[..., a, b] → [..., a>=b ? 1 : 0]` | Greater-or-equal. |

---

### Logical Boolean

Operands are treated as booleans: `0` is false, any non-zero value is true. Results are `1` or `0`.

| Code | Mnemonic | Stack effect | Interpretation |
|------|----------|--------------|----------------|
| `0x36` | `NOT` | `[..., a] → [..., a==0 ? 1 : 0]` | Logical NOT. |
| `0x37` | `AND` | `[..., a, b] → [..., (a!=0 && b!=0) ? 1 : 0]` | Logical AND. |
| `0x38` | `OR` | `[..., a, b] → [..., (a!=0 \|\| b!=0) ? 1 : 0]` | Logical OR. |
| `0x39` | `XOR` | `[..., a, b] → [..., ((a!=0) ^ (b!=0)) ? 1 : 0]` | Logical XOR. True iff exactly one operand is non-zero. |

---

### Bitwise

Operate on raw integer bit patterns.

| Code | Mnemonic | Stack effect | Interpretation |
|------|----------|--------------|----------------|
| `0x3A` | `BAND` | `[..., a, b] → [..., a & b]` | Bitwise AND. |
| `0x3B` | `BOR` | `[..., a, b] → [..., a \| b]` | Bitwise OR. |
| `0x3C` | `BXOR` | `[..., a, b] → [..., a ^ b]` | Bitwise XOR. |
| `0x3D` | `BNOT` | `[..., a] → [..., ~a]` | Bitwise NOT (one's complement). |
| `0x3E` | `SHL` | `[..., a, b] → [..., a << b]` | Left shift. |
| `0x3F` | `SHR` | `[..., a, b] → [..., a >> b]` | Right shift (arithmetic, sign-extending). |

> **Design note — shift opcodes.**
> `SHR` is arithmetic (sign-extending), not logical (zero-filling).
> Because the VM's type system is entirely signed integers, arithmetic
> right shift is the natural default — it preserves sign and is equivalent
> to integer division by 2^b. A separate logical right shift opcode is
> intentionally omitted: there is no unsigned integer type, no fixed bit
> width to zero-fill from, and no current workload that requires unsigned
> bit-pattern manipulation. Where logical right shift semantics are
> needed, they can be emulated with a mask (`BAND`) followed by `SHR`.

---

### Allocators

These instructions allocate typed objects into registers.

#### Model Allocators

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x40` | `BQMX` | `reg` | `[..., size] → [...]` | `write` — `reg ← xqmx(model, binary, size)` | Pop `size`. Create a binary `[0,1]` model XQMX with `size` variables and empty linear/quadratic tables. Write to `reg`. |
| `0x41` | `SQMX` | `reg` | `[..., size] → [...]` | `write` — `reg ← xqmx(model, spin, size)` | Pop `size`. Create a spin `[-1,+1]` model XQMX. Write to `reg`. |
| `0x42` | `XQMX` | `reg` | `[..., size, k] → [...]` | `write` — `reg ← xqmx(model, discrete(k), size)` | Pop `k`, then `size`. Create a discrete `[-k,...,k-1]` model XQMX. Error if `k < 2`. Write to `reg`. |

#### Sample Allocators

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x43` | `BSMX` | `reg` | `[..., size] → [...]` | `write` — `reg ← xqmx(sample, binary, size)` | Pop `size`. Create a binary `[0,1]` sample XQMX with `size` variables; every position initialised to `0`. Linear table stores variable assignments. Write to `reg`. |
| `0x44` | `SSMX` | `reg` | `[..., size] → [...]` | `write` — `reg ← xqmx(sample, spin, size)` | Pop `size`. Create a spin `[-1,+1]` sample XQMX; every position initialised to `-1` (a valid spin state). Write to `reg`. |
| `0x45` | `XSMX` | `reg` | `[..., size, k] → [...]` | `write` — `reg ← xqmx(sample, discrete(k), size)` | Pop `k`, then `size`. Create a discrete `[-k,...,k-1]` sample XQMX; every position initialised to `0`. Error if `k < 2`. Write to `reg`. |

Sample allocation is dense: after `BSMX`/`SSMX`/`XSMX` every position `i` in `[0, size)` holds its domain-default value. Reads via `GETLINE` see that default until a matching write overrides it. This mirrors the Rust runtime's `vec![default; size]` storage; the Python reference VM pre-populates the equivalent sparse entries (QUI-453).

#### Vec Allocators

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x4A` | `VEC` | `reg` | `[...] → [...]` | `write` — `reg ← vec(unset)` | Create an empty vec with unset element type (inferred on first push). Write to `reg`. |
| `0x4B` | `VECI` | `reg` | `[...] → [...]` | `write` — `reg ← vec<int>` | Create an empty `vec<int>`. Write to `reg`. |
| `0x4C` | `VECX` | `reg` | `[...] → [...]` | `write` — `reg ← vec<xqmx>` | Create an empty `vec<xqmx>`. Write to `reg`. |

---

### Vector Access

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x50` | `VECPUSH` | `reg` | `[..., v] → [...]` | `mutate` — appends `v` to `reg`'s vec | Pop `v`. `reg` must hold a vec. Append `v`. If vec type is unset, infer from `v`. Otherwise validate type compatibility. Error: `TypeMismatch` if `reg` is not a vec or if `v` has incompatible type. |
| `0x51` | `VECGET` | `reg` | `[..., idx] → [..., v]` | `read` — `reg` must hold `vec<int>` | Pop `idx`. `reg` must hold a vec. Bounds-check: `0 ≤ idx < len`. Push `vec[idx]`. Error: `IndexError` if out of bounds. Error: `TypeMismatch` if element is not int (cannot push non-int to stack). |
| `0x52` | `VECSET` | `reg` | `[..., idx, v] → [...]` | `mutate` — sets `reg.vec[idx] ← v` | Pop `v`, then `idx`. `reg` must hold a vec. Bounds-check: `0 ≤ idx < len`. Set `vec[idx] ← v`. Validates type compatibility. |
| `0x53` | `VECLEN` | `reg` | `[...] → [..., n]` | `read` — `reg` must hold a vec | `reg` must hold a vec. Push `len(vec)` as integer. Error: `TypeMismatch` if `reg` is not a vec. |

---

### Index Math

Utilities for mapping 2-D coordinates to flat array indices.

| Code | Mnemonic | Stack effect | Interpretation |
|------|----------|--------------|----------------|
| `0x5A` | `IDXGRID` | `[..., row, col, cols] → [..., row*cols+col]` | Row-major flat index. Pop `cols`, then `col`, then `row`. Push `row * cols + col`. |
| `0x5B` | `IDXTRIU` | `[..., i, j] → [..., j*(j-1)/2+i]` | Upper-triangular index for the pair `(i, j)`. Pop `j`, then `i`. If `i > j`, swap them. Push `j * (j - 1) / 2 + i`. |

---

### XQMX Coefficient Access

Read and write the linear (bias) and quadratic (coupling) coefficients of an XQMX register. Missing entries read as `0`; writes create the entry on first call. Zero values are removed from sparse storage to maintain sparsity. `reg` must hold an XQMX.

The linear opcodes (`GETLINE`, `SETLINE`, `ADDLINE`) accept either MODEL or SAMPLE mode — sample values are stored densely in `values[i]`, model biases sparsely in `linear[i]`. The quadratic opcodes (`GETQUAD`, `SETQUAD`, `ADDQUAD`) require MODEL mode: samples carry no quadratic storage, and `reg` must hold an XQMX in MODEL mode; error `XQMXModeError` otherwise.

#### Linear Coefficients

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x60` | `GETLINE` | `reg` | `[..., i] → [..., linear[i]]` | `read` — `reg.xqmx.linear[i]` | Pop `i`. Push `linear[i]` (0 if absent). |
| `0x61` | `SETLINE` | `reg` | `[..., i, v] → [...]` | `mutate` — `reg.xqmx.linear[i] ← v` | Pop `v`, then `i`. Set `linear[i] ← v`. Error: `IndexError` if `i` out of range `[0, size)`. |
| `0x62` | `ADDLINE` | `reg` | `[..., i, δ] → [...]` | `mutate` — `reg.xqmx.linear[i] += δ` | Pop `δ`, then `i`. `linear[i] += δ`. Error: `IndexError` if `i` out of range. |

#### Quadratic Coefficients

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x63` | `GETQUAD` | `reg` | `[..., i, j] → [..., quad[i,j]]` | `read` — `reg.xqmx.quad[i,j]` | Pop `j`, then `i`. If `i > j`, swap. Push `quad[i,j]` (0 if absent). |
| `0x64` | `SETQUAD` | `reg` | `[..., i, j, v] → [...]` | `mutate` — `reg.xqmx.quad[i,j] ← v` | Pop `v`, then `j`, then `i`. If `i > j`, swap. Set `quad[i,j] ← v`. Error: `IndexError` if indices out of range `[0, size)`. |
| `0x65` | `ADDQUAD` | `reg` | `[..., i, j, δ] → [...]` | `mutate` — `reg.xqmx.quad[i,j] += δ` | Pop `δ`, then `j`, then `i`. If `i > j`, swap. `quad[i,j] += δ`. Error: `IndexError` if indices out of range. |

---

### XQMX Grid

An XQMX register (model or sample) can optionally be given 2-D grid dimensions so that variables are addressed as `(row, col)` with flat index `row * cols + col`. `reg` must hold an XQMX. These opcodes accept either MODEL or SAMPLE mode — row/column reads scan the register's `linear` surface (models: sparse `linear[idx]`; samples: dense `values[idx]`).

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x66` | `RESIZE` | `reg` | `[..., rows, cols] → [...]` | `mutate` — `reg.xqmx.rows ← rows; reg.xqmx.cols ← cols` | Pop `cols`, then `rows`. Set grid dimensions on the XQMX. |
| `0x67` | `ROWFIND` | `reg` | `[..., row, value] → [..., col]` | `read` — scans `reg.xqmx.linear` across row | Pop `value`, then `row`. Scan `linear[row*cols + c]` for `c` in `0..cols`. Push the column index of the first entry equal to `value`, or `-1` if not found. |
| `0x68` | `COLFIND` | `reg` | `[..., col, value] → [..., row]` | `read` — scans `reg.xqmx.linear` down column | Pop `value`, then `col`. Scan `linear[r*cols + col]` for `r` in `0..rows`. Push the row index of the first match, or `-1` if not found. |
| `0x69` | `ROWSUM` | `reg` | `[..., row] → [..., sum]` | `read` — sums `reg.xqmx.linear` across row | Pop `row`. Push `Σ linear[row*cols + c]` for `c` in `0..cols`. |
| `0x6A` | `COLSUM` | `reg` | `[..., col] → [..., sum]` | `read` — sums `reg.xqmx.linear` down column | Pop `col`. Push `Σ linear[r*cols + col]` for `r` in `0..rows`. |

---

### XQMX High-Level Functions

These instructions inject QUBO penalty terms for common combinatorial constraints, expanding into linear and quadratic coefficient deltas automatically. `reg` must hold an XQMX in MODEL mode. Error: `XQMXModeError` if the XQMX is in SAMPLE mode.

| Code | Mnemonic | Arguments | Stack effect | Register effect | Interpretation |
|------|----------|-----------|--------------|-----------------|----------------|
| `0x70` | `ONEHOTR` | `reg` | `[..., row, penalty] → [...]` | `mutate` — adds to linear and quadratic for row variables | Pop `penalty`, then `row`. Apply one-hot constraint over all variables in grid row `row`. Grid dimensions must be set. See expansion below. |
| `0x71` | `ONEHOTC` | `reg` | `[..., col, penalty] → [...]` | `mutate` — adds to linear and quadratic for column variables | Pop `penalty`, then `col`. Apply one-hot constraint over all variables in grid column `col`. Grid dimensions must be set. See expansion below. |
| `0x72` | `EXCLUDE` | `reg` | `[..., i, j, penalty] → [...]` | `mutate` — `reg.xqmx.quad[i,j] += penalty` | Pop `penalty`, then `j`, then `i`. Add mutual-exclusion penalty. See expansion below. |
| `0x73` | `IMPLIES` | `reg` | `[..., i, j, penalty] → [...]` | `mutate` — modifies linear and quadratic | Pop `penalty`, then `j`, then `i`. Add implication constraint `i → j`. See expansion below. |
| `0x7F` | `ENERGY` | `model sample` | `[...] → [..., E]` | `read` — both `model` and `sample` registers are read-only | The `model` register must hold an XQMX in MODEL mode. The `sample` register must hold an XQMX in SAMPLE mode. Sizes must match. Compute and push the Hamiltonian energy. See formula below. |

#### `ONEHOTR` / `ONEHOTC` Expansion

Apply the one-hot constraint over a set of variable indices (all variables in a row or column):

```
H += penalty × (Σ x_i - 1)²
```

Expanding for binary variables (`x² = x`):

```
linear[i]    += -penalty          for each i in indices
quad[i, j]   += 2 × penalty      for each pair i < j in indices
```

`ONEHOTR` uses indices `[row*cols, row*cols+1, ..., row*cols+cols-1]`.
`ONEHOTC` uses indices `[col, col+cols, col+2*cols, ..., col+(rows-1)*cols]`.

#### `EXCLUDE` Expansion

Penalise `x_i = 1` and `x_j = 1` simultaneously:

```
quad[i, j] += penalty
```

#### `IMPLIES` Expansion

Penalise `x_i = 1` with `x_j = 0` (implication `x_i → x_j`):

```
H += penalty × x_i × (1 - x_j) = penalty × x_i - penalty × x_i × x_j

linear[i]    += penalty
quad[i, j]   += -penalty
```

#### `ENERGY` Computation

```
E = Σ_i linear_model[i] × x_sample[i]
  + Σ_{i<j} quad_model[i,j] × x_sample[i] × x_sample[j]
```

Where `x_sample[i] = sample.linear[i]` (the variable assignment). Error: `ValueError` if `model.size != sample.size`.

---

## Assembly Syntax

Line comments begin with `;` and run to end of line. Whitespace is insignificant outside operands.

```assembly
; Comments start with ;
; Registers: r0, r1, ... r255 (8-bit slot ID)
; Targets: .0, .1, .2 (dot-prefixed numeric, sugar for TARGET; resolved to sequential IDs by assembler)
; Hex literals: 0x0A, 0xFF

PUSH 0x00         ; start = 0
PUSH 0x0A         ; count = 10
RANGE             ; range loop [0, 10)
  LVAL r0         ; r0 = current loop value
  LOAD r0
  PUSH 0x05
  GT
  JUMPI .0        ; break if r0 > 5
NEXT
TARGET .0
HALT
```

### `PUSH` Sugar

The assembler accepts `PUSH <value>` as syntactic sugar for the `PUSH1`–`PUSH8` family. The assembler parses the signed integer, selects the smallest `PUSHn` opcode that fits, and encodes the value as big-endian signed two's complement byte operands. The desugared forms (`PUSH1 0xFF`, `PUSH2 0x01 0x00`, etc.) remain valid.

```assembly
PUSH 42            ; → PUSH1 42
PUSH -1            ; → PUSH1 0xFF
PUSH 256           ; → PUSH2 0x01 0x00
PUSH 2147483647    ; → PUSH4 0x7F 0xFF 0xFF 0xFF
```

### `JUMP` / `JUMPI` Sugar

The assembler accepts `JUMP .N` and `JUMPI .N` as syntactic sugar for the `JUMP1`/`JUMP2` and `JUMPI1`/`JUMPI2` pairs. The assembler resolves `.N` to the sequential target ID and selects the `*1` (u8) form when the ID fits in one byte, or the `*2` (u16 big-endian) form otherwise. The desugared forms remain valid.

---

## Binary Bytecode Format

A program is encoded as a raw byte stream with no header, magic bytes, length prefix, or alignment padding. Each instruction is laid out as:

```
[ opcode : 1 byte ] [ operand bytes : 0 – 8 bytes ]
```

Instruction length is determined solely by the opcode (see the [Instruction Length Table](#instruction-length-table)). Multi-operand opcodes concatenate their operands in the order listed in the main opcode tables.

### Opcode Byte

A single byte drawn from:

- `0x00`–`0x7F` — the normal instruction space
- `0xF0` — `NOP`
- `0xFF` — `HALT`

All other bytes in `0x80`–`0xFF` are reserved and must be rejected by the decoder.

**`TARGET` (`0x00`) has no operand.** The one-byte opcode is the entire instruction. `TARGET` marks a jump destination in the instruction stream and is a no-op at runtime; its only purpose is to establish the target table during pre-scan. Each `TARGET` encountered in program order is assigned the next sequential ID (0, 1, 2, …) and registered at its PC. The `.N` label seen in assembly source (e.g. `TARGET .3`) is assembler-only syntax: the assembler uses it to resolve `JUMP`/`JUMPI` references; it is **never emitted into bytecode**.

### Operand Bytes

| Operand | Width | Encoding | Used by |
|---------|-------|----------|---------|
| Register | 1 byte | `u8`, range `0`–`255` | all register-taking opcodes |
| Label (u8) | 1 byte | `u8`, range `0`–`255` | `JUMP1`, `JUMPI1` |
| Label (u16) | 2 bytes | `u16` big-endian, range `0`–`65535` | `JUMP2`, `JUMPI2` |
| Immediate | `n` bytes (`n` = 1..8) | big-endian signed two's complement | `PUSH1`–`PUSH8` |

Label operands encode the **sequential target ID produced by the `TARGET` pre-scan** — not a PC offset, and not the source-level `.N` token. The assembler emits `JUMP1`/`JUMPI1` when the ID fits in one byte and `JUMP2`/`JUMPI2` otherwise (see the [`JUMP` / `JUMPI` sugar](#jump--jumpi-sugar) description).

`ENERGY` is the only opcode with two register operands; they are concatenated in order `[ model_reg, sample_reg ]`. For example `ENERGY r0 r1 → 0x7F 0x00 0x01`.

### Instruction Length Table

| Length | Opcodes |
|--------|---------|
| 1 byte (opcode only) | `TARGET`, `NEXT`, `RANGE`, `NOP`, `HALT`, `POP`, `SCLR`, `SWAP`, `COPY`, `ADD`, `SUB`, `MUL`, `DIV`, `MOD`, `SQR`, `ABS`, `NEG`, `MIN`, `MAX`, `INC`, `DEC`, `EQ`, `LT`, `GT`, `LTE`, `GTE`, `NOT`, `AND`, `OR`, `XOR`, `BAND`, `BOR`, `BXOR`, `BNOT`, `SHL`, `SHR`, `IDXGRID`, `IDXTRIU` |
| 2 bytes (opcode + 1) | `JUMP1` (1 + u8 label), `JUMPI1` (1 + u8 label), `LIDX`, `LVAL`, `ITER`, `LOAD`, `STOW`, `DROP`, `INPUT`, `OUTPUT`, `PUSH1`, `VEC`, `VECI`, `VECX`, `BQMX`, `SQMX`, `XQMX`, `BSMX`, `SSMX`, `XSMX`, `VECPUSH`, `VECGET`, `VECSET`, `VECLEN`, `GETLINE`, `SETLINE`, `ADDLINE`, `GETQUAD`, `SETQUAD`, `ADDQUAD`, `RESIZE`, `ROWFIND`, `COLFIND`, `ROWSUM`, `COLSUM`, `ONEHOTR`, `ONEHOTC`, `EXCLUDE`, `IMPLIES` |
| 3 bytes (opcode + 2) | `JUMP2` (1 + u16 label), `JUMPI2` (1 + u16 label), `PUSH2` (1 + 2 imm), `ENERGY` (1 + 2 reg) |
| 4 bytes | `PUSH3` |
| 5 bytes | `PUSH4` |
| 6 bytes | `PUSH5` |
| 7 bytes | `PUSH6` |
| 8 bytes | `PUSH7` |
| 9 bytes | `PUSH8` |

> `XQMX` and `XSMX` take one register operand (2 bytes on the wire) but additionally pop two values from the stack. Their bytecode length is 2, not 3 — the popped stack values are not part of the encoding.

### Encoding Examples

```
NOP                  → 0xF0
HALT                 → 0xFF
TARGET               → 0x00                (no operand; .N is assembler-only)
PUSH1 42             → 0x11 0x2A
PUSH2 -1             → 0x12 0xFF 0xFF
LOAD r5              → 0x0A 0x05
JUMP1 .100           → 0x01 0x64           (JUMP .100 → JUMP1 — label fits in u8)
JUMP2 .1000          → 0x03 0x03 0xE8      (JUMP .1000 → JUMP2 — u16 big-endian)
JUMPI1 .5            → 0x02 0x05
ENERGY r0 r1         → 0x7F 0x00 0x01
BQMX r2              → 0x40 0x02
```

---

## Runtime Limits

| Limit | Value | Notes |
|-------|-------|-------|
| Stack depth | 8192 (2^13) | `StackOverflow` if exceeded. |
| Stack value range | `[-2^63, 2^63 - 1]` | Signed 64-bit. `ArithmeticOverflow` if exceeded. |
| Register slots | 256 (r0–r255) | 8-bit addressing. |
| Target IDs | 0–65535 | `u8` via `JUMP1`/`JUMPI1`, `u16` big-endian via `JUMP2`/`JUMPI2`. Sequential assignment during pre-scan. |
| Loop nesting | Unbounded | Limited by available memory. |
| XQMX size | Implementation-defined | No spec limit. |
| Program length | Implementation-defined | No spec limit. |

---

## Reserved Opcodes

The following byte values within `0x00`–`0x7F` are unassigned and reserved for future use. A decoder encountering any of these in opcode position must reject the program:

`0x08`, `0x0D`, `0x19`, `0x1D`, `0x1E`, `0x1F`, `0x2C`, `0x2D`, `0x2E`, `0x2F`, `0x35`, `0x46`, `0x47`, `0x48`, `0x49`, `0x4D`, `0x4E`, `0x4F`, `0x54`, `0x55`, `0x56`, `0x57`, `0x58`, `0x59`, `0x5C`, `0x5D`, `0x5E`, `0x5F`, `0x6B`, `0x6C`, `0x6D`, `0x6E`, `0x6F`, `0x74`, `0x75`, `0x76`, `0x77`, `0x78`, `0x79`, `0x7A`, `0x7B`, `0x7C`, `0x7D`, `0x7E`

All other byte values in `0x80`–`0xFF` are likewise reserved and illegal. The only valid opcodes outside `0x00`–`0x7F` are `0xF0` (`NOP`) and `0xFF` (`HALT`).
