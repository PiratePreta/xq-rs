# Instruction Set Reference

This section documents all 93 XQVM instructions, organised by category. Each
instruction page includes the opcode byte, mnemonic, operands, stack effect,
register effect, and a prose description.

## Notation

- **Stack diagrams** -- \\([\ldots, a, b] \to [\ldots, r]\\), where rightmost = **top**.
  \\(b\\) is popped first.
- **`reg`** -- the `u8` operand encoded in the instruction byte stream,
  identifying a register slot (r0--r255).
- **`label`** -- a `u16` index into the jump table.
- Assignments use \\(\leftarrow\\) (register write) and \\(\to\\) (stack push).
- **Iverson brackets** -- \\([P]\\) equals \\(1\\) if \\(P\\) is true, \\(0\\) otherwise.
- **Wrapping** -- all integer arithmetic uses wrapping semantics on `i64`
  (no panic on overflow; result truncated to 64 bits).

### Register Effect Modes

- **`read`** -- register contents are inspected but not changed.
- **`write`** -- register is replaced wholesale with a new value.
- **`mutate`** -- register's existing value is modified in-place (e.g.
  appending to a vec, incrementing a coefficient).

## `RegVal` -- The Register Value Type

Each of the 256 registers holds one variant of `RegVal`:

| Variant | Rust Type | Notes |
|---------|-----------|-------|
| `Int(i64)` | `i64` | Default value for every register. |
| `VecInt(Vec<i64>)` | `Vec<i64>` | Integer vector. |
| `VecXqmx(Vec<XqmxModel>)` | `Vec<XqmxModel>` | Vector of models. |
| `Model(XqmxModel)` | struct | QUBO/Ising/discrete Hamiltonian. |
| `Sample(XqmxSample)` | struct | Variable-assignment vector. |

Type mismatches at runtime produce a `RegisterType` error with the expected and
actual variant names.

## Reserved Opcodes

The following byte values are unassigned gaps; the decoder rejects them as
illegal:

`0x0D`, `0x19`, `0x35`

All other byte values outside the assigned ranges are likewise illegal.

## Quick Reference

| Code | Mnemonic | Category | One-Line Description |
|------|----------|----------|---------------------|
| `0x00` | `NOP` | [Control Flow](control-flow.md) | No operation |
| `0x01` | `TARGET` | [Control Flow](control-flow.md) | Mark valid jump destination |
| `0x02` | `JUMP2` | [Control Flow](control-flow.md) | Unconditional jump by u16 label (wide form) |
| `0x03` | `JUMPI2` | [Control Flow](control-flow.md) | Conditional jump (if non-zero) by u16 label (wide form) |
| `0x04` | `NEXT` | [Control Flow](control-flow.md) | Advance loop; jump back or exit |
| `0x05` | `LVAL` | [Control Flow](control-flow.md) | Copy loop value to register |
| `0x06` | `RANGE` | [Control Flow](control-flow.md) | Start range loop |
| `0x07` | `ITER` | [Control Flow](control-flow.md) | Start vec iteration |
| `0x08` | `LIDX` | [Control Flow](control-flow.md) | Copy loop index to register |
| `0x09` | `HALT` | [Control Flow](control-flow.md) | Stop execution |
| `0x0A` | `LOAD` | [Register I/O](register-io.md) | Push register int to stack |
| `0x0B` | `STOW` | [Register I/O](register-io.md) | Pop stack into register |
| `0x0C` | `DROP` | [Register I/O](register-io.md) | Reset register to Int(0) |
| `0x0E` | `INPUT` | [Register I/O](register-io.md) | Load calldata into register |
| `0x0F` | `OUTPUT` | [Register I/O](register-io.md) | Write register to output slot |
| `0x10` | `POP` | [Stack](stack-manipulation.md) | Discard top |
| `0x11`--`0x18` | `PUSH1`--`PUSH8` | [Stack](stack-manipulation.md) | Push 1--8 byte constant |
| `0x1A` | `SCLR` | [Stack](stack-manipulation.md) | Clear entire stack |
| `0x1B` | `SWAP` | [Stack](stack-manipulation.md) | Swap top two |
| `0x1C` | `COPY` | [Stack](stack-manipulation.md) | Duplicate top |
| `0x20` | `ADD` | [Arithmetic](arithmetic.md) | Wrapping addition |
| `0x21` | `SUB` | [Arithmetic](arithmetic.md) | Wrapping subtraction |
| `0x22` | `MUL` | [Arithmetic](arithmetic.md) | Wrapping multiplication |
| `0x23` | `DIV` | [Arithmetic](arithmetic.md) | Truncating division |
| `0x24` | `MOD` | [Arithmetic](arithmetic.md) | Truncating remainder |
| `0x25` | `SQR` | [Arithmetic](arithmetic.md) | Wrapping square |
| `0x26` | `ABS` | [Arithmetic](arithmetic.md) | Wrapping absolute value |
| `0x27` | `NEG` | [Arithmetic](arithmetic.md) | Wrapping negation |
| `0x28` | `MIN` | [Arithmetic](arithmetic.md) | Signed minimum |
| `0x29` | `MAX` | [Arithmetic](arithmetic.md) | Signed maximum |
| `0x2A` | `INC` | [Arithmetic](arithmetic.md) | Wrapping increment |
| `0x2B` | `DEC` | [Arithmetic](arithmetic.md) | Wrapping decrement |
| `0x2C` | `BITLEN` | [Arithmetic](arithmetic.md) | Bit length of non-negative int |
| `0x30` | `EQ` | [Comparison](comparison.md) | Equality |
| `0x31` | `LT` | [Comparison](comparison.md) | Less-than |
| `0x32` | `GT` | [Comparison](comparison.md) | Greater-than |
| `0x33` | `LTE` | [Comparison](comparison.md) | Less-or-equal |
| `0x34` | `GTE` | [Comparison](comparison.md) | Greater-or-equal |
| `0x36` | `NOT` | [Logical](logical.md) | Logical NOT |
| `0x37` | `AND` | [Logical](logical.md) | Logical AND |
| `0x38` | `OR` | [Logical](logical.md) | Logical OR |
| `0x39` | `XOR` | [Logical](logical.md) | Logical XOR |
| `0x3A` | `BAND` | [Bitwise](bitwise.md) | Bitwise AND |
| `0x3B` | `BOR` | [Bitwise](bitwise.md) | Bitwise OR |
| `0x3C` | `BXOR` | [Bitwise](bitwise.md) | Bitwise XOR |
| `0x3D` | `BNOT` | [Bitwise](bitwise.md) | Bitwise NOT |
| `0x3E` | `SHL` | [Bitwise](bitwise.md) | Left shift |
| `0x3F` | `SHR` | [Bitwise](bitwise.md) | Arithmetic right shift |
| `0x40` | `BQMX` | [Allocators](allocators.md) | Alloc binary QUBO model |
| `0x41` | `SQMX` | [Allocators](allocators.md) | Alloc spin Ising model |
| `0x42` | `XQMX` | [Allocators](allocators.md) | Alloc discrete model |
| `0x43` | `BSMX` | [Allocators](allocators.md) | Alloc binary sample |
| `0x44` | `SSMX` | [Allocators](allocators.md) | Alloc spin sample |
| `0x45` | `XSMX` | [Allocators](allocators.md) | Alloc discrete sample |
| `0x4A` | `VEC` | [Allocators](allocators.md) | Create empty vec |
| `0x4B` | `VECI` | [Allocators](allocators.md) | Create empty vec\<int\> |
| `0x4C` | `VECX` | [Allocators](allocators.md) | Create empty vec\<xqmx\> |
| `0x50` | `VECPUSH` | [Vector Operations](vector-ops.md) | Append to vec |
| `0x51` | `VECGET` | [Vector Operations](vector-ops.md) | Read vec element |
| `0x52` | `VECSET` | [Vector Operations](vector-ops.md) | Write vec element |
| `0x53` | `VECLEN` | [Vector Operations](vector-ops.md) | Push vec length |
| `0x54` | `SLACK` | [Vector Operations](vector-ops.md) | Generate slack variable entries |
| `0x5A` | `IDXGRID` | [Index Math](index-math.md) | Row-major flat index |
| `0x5B` | `IDXTRIU` | [Index Math](index-math.md) | Upper-triangular index |
| `0x60` | `GETLINE` | [Coefficient Access](coefficient-access.md) | Get linear coefficient |
| `0x61` | `SETLINE` | [Coefficient Access](coefficient-access.md) | Set linear coefficient |
| `0x62` | `ADDLINE` | [Coefficient Access](coefficient-access.md) | Add to linear coefficient |
| `0x63` | `GETQUAD` | [Coefficient Access](coefficient-access.md) | Get quadratic coefficient |
| `0x64` | `SETQUAD` | [Coefficient Access](coefficient-access.md) | Set quadratic coefficient |
| `0x65` | `ADDQUAD` | [Coefficient Access](coefficient-access.md) | Add to quadratic coefficient |
| `0x66` | `RESIZE` | [Grid](grid.md) | Set grid dimensions |
| `0x67` | `ROWFIND` | [Grid](grid.md) | Find value in row |
| `0x68` | `COLFIND` | [Grid](grid.md) | Find value in column |
| `0x69` | `ROWSUM` | [Grid](grid.md) | Sum row coefficients |
| `0x6A` | `COLSUM` | [Grid](grid.md) | Sum column coefficients |
| `0x70` | `ONEHOTR` | [Constraints](constraints.md) | One-hot over row |
| `0x71` | `ONEHOTC` | [Constraints](constraints.md) | One-hot over column |
| `0x72` | `EXCLUDE` | [Constraints](constraints.md) | Mutual exclusion |
| `0x73` | `IMPLIES` | [Constraints](constraints.md) | Implication constraint |
| `0x74` | `EQUALITY` | [Constraints](constraints.md) | Weighted equality constraint |
| `0x75` | `ATLEAST` | [Constraints](constraints.md) | At-least-k constraint |
| `0x76` | `ATLEASTW` | [Constraints](constraints.md) | Weighted at-least-k constraint |
| `0x77` | `REDUCE` | [Constraints](constraints.md) | HOBO degree reduction |
| `0x7F` | `ENERGY` | [Energy](energy.md) | Evaluate Hamiltonian |
| `0x80` | `JUMP1` | [Control Flow](control-flow.md) | Unconditional jump by u8 label (narrow form) |
| `0x81` | `JUMPI1` | [Control Flow](control-flow.md) | Conditional jump (if non-zero) by u8 label (narrow form) |
