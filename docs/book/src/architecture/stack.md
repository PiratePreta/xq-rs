# Operand Stack

The operand stack is the primary workspace for computation. It holds `i64`
signed 64-bit integers and is used by arithmetic, comparison, logical, and
bitwise instructions.

## Properties

| Property | Value |
|----------|-------|
| Element type | `i64` (signed 64-bit integer) |
| Maximum depth | 8,192 items |
| Ordering | LIFO (last in, first out) |
| Initial state | Empty |

## Operations

- **Push** -- `PUSH1`--`PUSH8` push constants. `LOAD` pushes a register's
  integer value. `COPY` duplicates the top element.
- **Pop** -- most instructions implicitly pop their operands. `POP` explicitly
  discards the top element.
- **Swap** -- `SWAP` exchanges the top two elements.
- **Clear** -- `SCLR` removes all elements.

## Stack Diagrams

Throughout this documentation, stack effects are written as:

$$[\ldots, a, b] \to [\ldots, r]$$

- \\(\ldots\\) represents elements below the operands.
- Rightmost = **top** of stack.
- \\(b\\) is popped first (it was pushed last).
- \\(r\\) is the result pushed after the operation.

## Errors

- **`StackUnderflow`** -- popping from an empty stack or when there are fewer
  elements than the instruction requires.
- **`StackOverflow`** -- pushing when the stack already contains 8,192 items.

## Interaction with Registers

The stack holds only `i64` integers. Richer types (models, vectors, samples)
live exclusively in registers. The bridge between them:

- `LOAD reg` -- pushes a register's `Int` value onto the stack.
- `STOW reg` -- pops a stack value into a register as `Int`.

To move non-integer values, use `INPUT`/`OUTPUT` with calldata and output slots.
