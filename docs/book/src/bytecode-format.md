# Bytecode Format

This chapter describes the binary wire format of XQVM programs.

## Program Structure

A compiled program is a contiguous byte buffer with two sections:

```mermaid
block-beta
  columns 2
  A["Jump Table\n(2 + 10·N bytes)"] B["Instruction Stream\n(variable length)"]
```

There is no magic number, version field, or checksum. The jump table is always
present, even if empty (minimum 2 bytes for the entry count).

## Jump Table

The jump table maps label indices to byte offset ranges in the instruction
stream.

### Header

| Field | Type | Size | Description |
|-------|------|------|-------------|
| `entry_count` | `u16 BE` | 2 bytes | Number of jump table entries. |

### Per Entry (10 bytes each)

| Field | Type | Size | Description |
|-------|------|------|-------------|
| `label` | `u16 BE` | 2 bytes | Zero-based label index. |
| `start` | `u32 BE` | 4 bytes | Byte offset (inclusive) of the basic block start in the instruction stream. |
| `end` | `u32 BE` | 4 bytes | Byte offset (exclusive) of the basic block end. |

**Total jump table size:** `2 + 10 * entry_count` bytes.

A program with no labels has a 2-byte jump table (`entry_count = 0`).

### Byte Offset Reference

The `start` and `end` offsets are relative to the **instruction stream**, not
the entire program buffer. That is, offset `0` refers to the first byte after
the jump table.

## Instruction Encoding

Each instruction is encoded as:

```
[opcode: u8] [operand_0] [operand_1] ...
```

All multi-byte values are **big-endian**.

### Operand Types

| Type | Wire Size | Encoding |
|------|-----------|----------|
| Register | 1 byte | Raw `u8` slot index (0--255). |
| Label | 2 bytes | `u16 BE` jump table index. |
| `[u8; N]` (PUSH) | N bytes | Big-endian signed integer (1 ≤ N ≤ 8). Sign-extended to `i64` on decode. |

### Instruction Sizes

| Instruction | Total Bytes | Layout |
|-------------|-------------|--------|
| No-operand (NOP, HALT, ADD, POP, ...) | 1 | `[opcode]` |
| Single register (LOAD, STOW, BQMX, ...) | 2 | `[opcode, reg]` |
| Label (JUMP, JUMPI) | 3 | `[opcode, label_hi, label_lo]` |
| PUSH1 | 2 | `[0x11, val]` |
| PUSH2 | 3 | `[0x12, val_hi, val_lo]` |
| PUSH3--PUSH7 | 4--8 | `[opcode, val_bytes...]` |
| PUSH8 | 9 | `[0x18, 8 bytes BE]` |
| ENERGY | 3 | `[0x7F, model_reg, sample_reg]` |

### PUSH Size Selection

The assembler and `InstructionBuilder::push()` automatically select the smallest
encoding that faithfully represents the value:

| Value Range | Instruction | Wire Bytes |
|-------------|-------------|------------|
| -128 to 127 | PUSH1 | 2 |
| -32,768 to 32,767 | PUSH2 | 3 |
| fits in 24-bit signed | PUSH3 | 4 |
| fits in 32-bit signed | PUSH4 | 5 |
| fits in 40-bit signed | PUSH5 | 6 |
| fits in 48-bit signed | PUSH6 | 7 |
| fits in 56-bit signed | PUSH7 | 8 |
| any `i64` | PUSH8 | 9 |

The constant is stored big-endian and sign-extended to `i64` on decode. For
example, `PUSH1 0xFF` decodes as `-1i64`.

## Example

Consider this program:

```asm
.0: TARGET
    PUSH 42
    JUMP .0
```

### Encoded Bytes

**Jump table** (12 bytes):
```
00 01           ; entry_count = 1
00 00           ; label = 0
00 00 00 00     ; start = 0
00 00 00 05     ; end = 5
```

**Instruction stream** (5 bytes):
```
01              ; TARGET (0x01)
11 2A           ; PUSH1 42 (0x11, 0x2A)
02 00 00        ; JUMP .0 (0x02, 0x00, 0x00)
```

**Total program:** 17 bytes.
