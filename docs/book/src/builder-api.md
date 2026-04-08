# Builder API

The `InstructionBuilder` provides a fluent Rust API for constructing XQVM
bytecode programmatically, without going through the text assembler.

## Basic Usage

```rust
use aglais_xqvm_bytecode::InstructionBuilder;

let mut b = InstructionBuilder::new();
b.push(10)
 .push(32)
 .add()
 .halt();

let program = b.build().unwrap();
```

## Labels

Labels are opaque handles. Create them with `label()`, anchor them with
`place()`, and reference them in `jump()`/`jump_if()`. Both forward and
backward references work.

### Backward Reference

```rust
use aglais_xqvm_bytecode::InstructionBuilder;

let mut b = InstructionBuilder::new();
let loop_top = b.label();

b.push(3);
b.place(loop_top);      // anchor label at this position
b.push(-1);
b.add();
b.copy();
b.jump_if(loop_top);    // backward jump to loop_top
b.pop();
b.halt();

let program = b.build().unwrap();
```

### Forward Reference

```rust
use aglais_xqvm_bytecode::InstructionBuilder;

let mut b = InstructionBuilder::new();
let done = b.label();

b.push(0);
b.jump_if(done);        // forward jump -- target not yet placed
b.push(42);
b.place(done);           // anchor here
b.halt();

let program = b.build().unwrap();
```

## PUSH Auto-Sizing

`push(val)` automatically selects the smallest `PUSH1`--`PUSH8` instruction:

```rust
b.push(0);         // emits PUSH1 (2 bytes)
b.push(42);        // emits PUSH1 (2 bytes)
b.push(1000);      // emits PUSH2 (3 bytes)
b.push(i64::MAX);  // emits PUSH8 (9 bytes)
```

## Register Operations

Most register instructions have a corresponding method:

```rust
use aglais_xqvm_bytecode::{InstructionBuilder, Register};

let mut b = InstructionBuilder::new();
b.push(42)
 .stow(Register(0))     // r0 ← Int(42)
 .load(Register(0))     // push r0's value
 .bqmx(Register(1))     // allocate QUBO model in r1
 .halt();
```

`DROP` is available as `drop_reg()` to avoid shadowing Rust's `core::mem::drop`:

```rust
b.drop_reg(Register(5));  // r5 ← Int(0)
```

## ENERGY

The `energy()` method takes two register operands:

```rust
b.energy(Register(0), Register(1));  // ENERGY r0 r1
```

## Raw Instruction Emit

For instructions without a dedicated method, use `emit()`:

```rust
use aglais_xqvm_bytecode::{InstructionBuilder, Instruction};

let mut b = InstructionBuilder::new();
b.emit(Instruction::Copy {})
 .emit(Instruction::Halt {});
```

## Build Errors

`build()` validates all labels and returns errors for:

- **`UnplacedLabel`** -- a label was used in a `JUMP`/`JUMPI` but never placed.
- **`UnusedLabel`** -- a label was placed but never referenced by any jump.

```rust
let mut b = InstructionBuilder::new();
let ghost = b.label();
b.jump(ghost).halt();
assert!(b.build().is_err());  // UnplacedLabel
```

## Jump Table Construction

`build()` automatically constructs the jump table from placed labels. Each
label becomes a jump table entry with a byte range `[start, end)` covering its
basic block. The jump table is included in the final `Program`.

```rust
let mut b = InstructionBuilder::new();
let l0 = b.label();
let l1 = b.label();
b.place(l0).nop().jump(l1).place(l1).halt();

let program = b.build().unwrap();
assert_eq!(program.jump_table().len(), 2);
```
