# Assembly Language

XQVM programs are written in a simple assembly language and stored in `.xqasm`
files. The assembler (`aglais-xqvm-asm` crate, invoked via `xq asm`) parses
the source, resolves labels, and emits compact bytecode.

## Overview

- Line-oriented format: one instruction per line.
- Comments start with `;` and run to end of line.
- Mnemonics are case-insensitive (`PUSH`, `push`, `Push` all work).
- Labels use numeric `.N` syntax (`.0`, `.1`, `.42`).
- Registers use `r<digits>` syntax (`r0`, `r255`).
- Integer literals may be signed decimal or `0x`-prefixed hexadecimal.

## Quick Example

```asm
; Compute 10 + 32 = 42
PUSH 10
PUSH 32
ADD
HALT
```

## Chapters

- [Syntax](syntax.md) -- full syntax reference
- [Assembly Examples](examples.md) -- annotated example programs
