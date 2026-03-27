# XQVM Bytecode Semantics

Authoritative reference for every instruction in the XQVM bytecode format.
Derived directly from `crates/bytecode/src/types/table.rs`, which is the single
source of truth: the `opcodes!` x-macro generates the `Opcode` and `Instruction`
enums, the codec, the assembler, the disassembler, and the VM dispatcher from
this one file.

## Notation

- **Stack effect** — described as `[..., a, b] → [...]` where the rightmost item
  is the **top** of the stack.
- **Register** — a `u8` index (`r0`–`r255`). Each slot holds one of:
  `Int(i64)`, `VecInt`, `VecXqmx`, `Model`, or `Sample`.
- **`label: u16`** — a jump-table index resolved at load time to a byte offset.
- **`val: [u8; N]`** — `N` inline bytes in big-endian order, sign-extended to
  `i64` on the stack.
- **Reserved** — byte codes `0x08`, `0x0D`, `0x19`, and `0x35` are unassigned
  gaps. The decoder treats them as illegal opcodes.

---

## Control Flow

| Code   | Mnemonic  | Arguments     | Interpretation                                                                                      |
|--------|-----------|---------------|-----------------------------------------------------------------------------------------------------|
| `0x00` | `NOP`     | —             | No operation.                                                                                       |
| `0x01` | `TARGET`  | —             | Mark a valid jump destination. Required before any label that `JUMP`/`JUMPI` can target.            |
| `0x02` | `JUMP`    | `label: u16`  | Unconditionally jump to the basic block at `label`.                                                 |
| `0x03` | `JUMPI`   | `label: u16`  | Pop `cond`; jump to `label` if `cond != 0`, otherwise fall through.                                 |
| `0x04` | `NEXT`    | —             | Advance the active loop counter. If the range/iteration is not exhausted, jump back to loop body start; otherwise pop the loop frame and fall through. |
| `0x05` | `LVAL`    | `reg: Register` | Copy the current loop value (range integer or vec element) into `reg`.                            |
| `0x06` | `RANGE`   | —             | Pop `count`, then `start`; start a range loop over `[start, start + count)`.                       |
| `0x07` | `ITER`    | `reg: Register` | Start a vec iteration over the vec held in `reg`.                                                 |
| `0x09` | `HALT`    | —             | Stop execution immediately.                                                                         |

---

## Register I/O

| Code   | Mnemonic  | Arguments       | Interpretation                                                                            |
|--------|-----------|-----------------|-------------------------------------------------------------------------------------------|
| `0x0A` | `LOAD`    | `reg: Register` | Push the integer value of `reg` onto the stack.                                           |
| `0x0B` | `STOW`    | `reg: Register` | Pop the top of the stack and store it into `reg`.                                         |
| `0x0C` | `DROP`    | `reg: Register` | Reset `reg` to `Int(0)`, releasing any heap allocation it held.                           |
| `0x0E` | `INPUT`   | `reg: Register` | Pop a calldata slot index `s`; load calldata slot `s` into `reg`.                        |
| `0x0F` | `OUTPUT`  | `reg: Register` | Pop an output slot index `s`; write `reg` to output slot `s`.                            |

---

## Stack Manipulation

| Code   | Mnemonic  | Arguments       | Interpretation                                                                            |
|--------|-----------|-----------------|-------------------------------------------------------------------------------------------|
| `0x10` | `POP`     | —               | Discard the top of the stack.                                                             |
| `0x11` | `PUSH1`   | `val: [u8; 1]`  | Push `val` sign-extended to `i64`.                                                        |
| `0x12` | `PUSH2`   | `val: [u8; 2]`  | Push `val` (big-endian, 2 bytes) sign-extended to `i64`.                                  |
| `0x13` | `PUSH3`   | `val: [u8; 3]`  | Push `val` (big-endian, 3 bytes) sign-extended to `i64`.                                  |
| `0x14` | `PUSH4`   | `val: [u8; 4]`  | Push `val` (big-endian, 4 bytes) sign-extended to `i64`.                                  |
| `0x15` | `PUSH5`   | `val: [u8; 5]`  | Push `val` (big-endian, 5 bytes) sign-extended to `i64`.                                  |
| `0x16` | `PUSH6`   | `val: [u8; 6]`  | Push `val` (big-endian, 6 bytes) sign-extended to `i64`.                                  |
| `0x17` | `PUSH7`   | `val: [u8; 7]`  | Push `val` (big-endian, 7 bytes) sign-extended to `i64`.                                  |
| `0x18` | `PUSH8`   | `val: [u8; 8]`  | Push the full 8-byte big-endian `i64` constant `val`.                                     |
| `0x1A` | `SCLR`    | —               | Clear the entire value stack.                                                             |
| `0x1B` | `SWAP`    | —               | Swap the top two stack elements.                                                          |
| `0x1C` | `COPY`    | —               | Duplicate the top of the stack.                                                           |

---

## Arithmetic

All arithmetic instructions operate on `i64` values. Pop order: `b` is popped first
(top of stack), then `a`.

| Code   | Mnemonic | Arguments | Interpretation                                       |
|--------|----------|-----------|------------------------------------------------------|
| `0x20` | `ADD`    | —         | Pop `b`, `a`; push `a + b`.                          |
| `0x21` | `SUB`    | —         | Pop `b`, `a`; push `a - b`.                          |
| `0x22` | `MUL`    | —         | Pop `b`, `a`; push `a * b`.                          |
| `0x23` | `DIV`    | —         | Pop `b`, `a`; push `a / b` (truncating). Errors if `b == 0`. |
| `0x24` | `MOD`    | —         | Pop `b`, `a`; push `a % b`. Errors if `b == 0`.     |
| `0x25` | `SQR`    | —         | Pop `a`; push `a * a`.                               |
| `0x26` | `ABS`    | —         | Pop `a`; push `|a|`.                                 |
| `0x27` | `NEG`    | —         | Pop `a`; push `-a`.                                  |
| `0x28` | `MIN`    | —         | Pop `b`, `a`; push `min(a, b)`.                      |
| `0x29` | `MAX`    | —         | Pop `b`, `a`; push `max(a, b)`.                      |
| `0x2A` | `INC`    | —         | Pop `a`; push `a + 1`.                               |
| `0x2B` | `DEC`    | —         | Pop `a`; push `a - 1`.                               |

---

## Comparison

Results are `1` (true) or `0` (false). Pop order: `b` first, then `a`.

| Code   | Mnemonic | Arguments | Interpretation                                  |
|--------|----------|-----------|-------------------------------------------------|
| `0x30` | `EQ`     | —         | Pop `b`, `a`; push `1` if `a == b`, else `0`.  |
| `0x31` | `LT`     | —         | Pop `b`, `a`; push `1` if `a < b`, else `0`.   |
| `0x32` | `GT`     | —         | Pop `b`, `a`; push `1` if `a > b`, else `0`.   |
| `0x33` | `LTE`    | —         | Pop `b`, `a`; push `1` if `a <= b`, else `0`.  |
| `0x34` | `GTE`    | —         | Pop `b`, `a`; push `1` if `a >= b`, else `0`.  |

---

## Logical Boolean

Operands are treated as booleans: zero is false, non-zero is true. Results are `1` or `0`.

| Code   | Mnemonic | Arguments | Interpretation                                               |
|--------|----------|-----------|--------------------------------------------------------------|
| `0x36` | `NOT`    | —         | Pop `a`; push `1` if `a == 0`, else `0`.                    |
| `0x37` | `AND`    | —         | Pop `b`, `a`; push `1` if both non-zero, else `0`.          |
| `0x38` | `OR`     | —         | Pop `b`, `a`; push `1` if either non-zero, else `0`.        |
| `0x39` | `XOR`    | —         | Pop `b`, `a`; push `1` if exactly one is non-zero, else `0`.|

---

## Bitwise

Operate on the raw `i64` bit patterns.

| Code   | Mnemonic | Arguments | Interpretation                                                  |
|--------|----------|-----------|-----------------------------------------------------------------|
| `0x3A` | `BAND`   | —         | Pop `b`, `a`; push `a & b`.                                    |
| `0x3B` | `BOR`    | —         | Pop `b`, `a`; push `a \| b`.                                   |
| `0x3C` | `BXOR`   | —         | Pop `b`, `a`; push `a ^ b`.                                    |
| `0x3D` | `BNOT`   | —         | Pop `a`; push `~a`.                                            |
| `0x3E` | `SHL`    | —         | Pop `b`, `a`; push `a << b`. `b` must be in `0..=63`.          |
| `0x3F` | `SHR`    | —         | Pop `b`, `a`; push `a >> b` (logical/unsigned). `b` must be in `0..=63`. |

---

## Allocators

These instructions allocate quantum/combinatorial model or sample objects into registers.

### Model Allocators

| Code   | Mnemonic | Arguments       | Interpretation                                                                 |
|--------|----------|-----------------|--------------------------------------------------------------------------------|
| `0x40` | `BQMX`   | `reg: Register` | Pop `size`; allocate a binary QUBO model (variable domain `{0, 1}`) into `reg`.         |
| `0x41` | `SQMX`   | `reg: Register` | Pop `size`; allocate a spin Ising model (variable domain `{-1, 1}`) into `reg`.         |
| `0x42` | `XQMX`   | `reg: Register` | Pop `k`, then `size`; allocate a discrete model (variable domain `{0, …, k-1}`) into `reg`. |

### Sample Allocators

| Code   | Mnemonic | Arguments       | Interpretation                                                                    |
|--------|----------|-----------------|-----------------------------------------------------------------------------------|
| `0x43` | `BSMX`   | `reg: Register` | Pop `size`; allocate a binary sample (domain `{0, 1}`) into `reg`.                       |
| `0x44` | `SSMX`   | `reg: Register` | Pop `size`; allocate a spin sample (domain `{-1, 1}`) into `reg`.                        |
| `0x45` | `XSMX`   | `reg: Register` | Pop `k`, then `size`; allocate a discrete sample (domain `{0, …, k-1}`) into `reg`. |

### Vec Allocators

| Code   | Mnemonic | Arguments       | Interpretation                                                                   |
|--------|----------|-----------------|----------------------------------------------------------------------------------|
| `0x4A` | `VEC`    | `reg: Register` | Create an empty vec in `reg`; element type is inferred on the first `VECPUSH`.   |
| `0x4B` | `VECI`   | `reg: Register` | Create an empty `vec<int>` in `reg`.                                             |
| `0x4C` | `VECX`   | `reg: Register` | Create an empty `vec<xqmx>` in `reg`.                                            |

---

## Vector Access

| Code   | Mnemonic   | Arguments       | Interpretation                                                        |
|--------|------------|-----------------|-----------------------------------------------------------------------|
| `0x50` | `VECPUSH`  | `reg: Register` | Pop `value`; append it to the vec in `reg`.                          |
| `0x51` | `VECGET`   | `reg: Register` | Pop `index`; push `vec[index]` from the vec in `reg`.                |
| `0x52` | `VECSET`   | `reg: Register` | Pop `value`, then `index`; set `vec[index]` in the vec in `reg`.     |
| `0x53` | `VECLEN`   | `reg: Register` | Push the length of the vec in `reg` onto the stack.                  |

---

## Index Math

Utilities for mapping 2-D coordinates to flat array indices.

| Code   | Mnemonic   | Arguments | Interpretation                                                                    |
|--------|------------|-----------|-----------------------------------------------------------------------------------|
| `0x5A` | `IDXGRID`  | —         | Pop `cols`, `col`, `row`; push `row * cols + col` (row-major flat index).        |
| `0x5B` | `IDXTRIU`  | —         | Pop `j`, `i` (where `i <= j`); push the upper-triangular index `j*(j-1)/2 + i`. |

---

## XQMX Coefficient Access

Read and write linear (bias) and quadratic (coupling) coefficients of a model. Missing
entries read as `0`; writes create the entry on first use.

### Linear Coefficients

| Code   | Mnemonic   | Arguments       | Interpretation                                                              |
|--------|------------|-----------------|-----------------------------------------------------------------------------|
| `0x60` | `GETLINE`  | `reg: Register` | Pop `i`; push `linear[i]` from `reg`'s model (0 if absent).               |
| `0x61` | `SETLINE`  | `reg: Register` | Pop `value`, `i`; set `linear[i]` in `reg`'s model to `value`.            |
| `0x62` | `ADDLINE`  | `reg: Register` | Pop `delta`, `i`; add `delta` to `linear[i]` in `reg`'s model.            |

### Quadratic Coefficients

| Code   | Mnemonic   | Arguments       | Interpretation                                                                    |
|--------|------------|-----------------|-----------------------------------------------------------------------------------|
| `0x63` | `GETQUAD`  | `reg: Register` | Pop `j`, `i`; push `quadratic[i, j]` from `reg`'s model (0 if absent).          |
| `0x64` | `SETQUAD`  | `reg: Register` | Pop `value`, `j`, `i`; set `quadratic[i, j]` in `reg`'s model to `value`.       |
| `0x65` | `ADDQUAD`  | `reg: Register` | Pop `delta`, `j`, `i`; add `delta` to `quadratic[i, j]` in `reg`'s model.       |

---

## XQMX Grid

A model can optionally be given 2-D grid dimensions, enabling row/column addressing
of variables.

| Code   | Mnemonic   | Arguments       | Interpretation                                                                           |
|--------|------------|-----------------|------------------------------------------------------------------------------------------|
| `0x66` | `RESIZE`   | `reg: Register` | Pop `cols`, `rows`; set the grid dimensions of `reg`'s model to `rows × cols`.          |
| `0x67` | `ROWFIND`  | `reg: Register` | Pop `value`, `row`; push the first column index where a linear value equals `value`, or `-1` if not found. |
| `0x68` | `COLFIND`  | `reg: Register` | Pop `value`, `col`; push the first row index where a linear value equals `value`, or `-1` if not found.   |
| `0x69` | `ROWSUM`   | `reg: Register` | Pop `row`; push the sum of all linear values in grid row `row`.                          |
| `0x6A` | `COLSUM`   | `reg: Register` | Pop `col`; push the sum of all linear values in grid column `col`.                       |

---

## XQMX High-Level Constraints

These instructions inject QUBO penalty terms for common combinatorial constraints,
expanding them into linear and quadratic coefficients automatically.

| Code   | Mnemonic   | Arguments       | Interpretation                                                                                                                         |
|--------|------------|-----------------|----------------------------------------------------------------------------------------------------------------------------------------|
| `0x70` | `ONEHOTR`  | `reg: Register` | Pop `penalty`, `row`; add a one-hot constraint over all variables in grid row `row`. Encodes `H += penalty * (Σxᵢ - 1)²`.             |
| `0x71` | `ONEHOTC`  | `reg: Register` | Pop `penalty`, `col`; add a one-hot constraint over all variables in grid column `col`. Encodes `H += penalty * (Σxᵢ - 1)²`.          |
| `0x72` | `EXCLUDE`  | `reg: Register` | Pop `penalty`, `j`, `i`; add a mutual-exclusion constraint: penalises `xᵢ = 1` and `xⱼ = 1` simultaneously. Encodes `H += penalty * xᵢxⱼ`. |
| `0x73` | `IMPLIES`  | `reg: Register` | Pop `penalty`, `j`, `i`; add an implication constraint `i → j`: penalises `xᵢ = 1` with `xⱼ = 0`. Encodes `H += penalty * xᵢ(1 - xⱼ)`. |

---

## Energy Evaluation

| Code   | Mnemonic  | Arguments                          | Interpretation                                                                   |
|--------|-----------|------------------------------------|----------------------------------------------------------------------------------|
| `0x7F` | `ENERGY`  | `model: Register, sample: Register` | Compute the Hamiltonian energy of `sample` evaluated against `model`; push the `i64` result. |

`ENERGY` is the only instruction with two register operands.

---

## Reserved / Illegal Opcodes

The following byte values are explicitly unassigned gaps in the table and will
cause a decode error at runtime:

`0x08`, `0x0D`, `0x19`, `0x35`

All other byte values outside the ranges above are likewise illegal.
