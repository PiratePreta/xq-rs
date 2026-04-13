# Control Flow

Instructions for branching, looping, and program termination.

| Code | Mnemonic | Arguments | Stack Effect | Register Effect | Description |
|------|----------|-----------|--------------|-----------------|-------------|
| `0x00` | `NOP` | -- | \\([\ldots] \to [\ldots]\\) | -- | No operation. |
| `0x01` | `TARGET` | -- | \\([\ldots] \to [\ldots]\\) | -- | Mark a valid jump destination. Required at every label that `JUMP`/`JUMPI` may target; treated as `NOP` at runtime. |
| `0x02` | `JUMP` | `label: u16` | \\([\ldots] \to [\ldots]\\) | -- | Seek the instruction stream to `jump_table[label].start`. Unconditional. |
| `0x03` | `JUMPI` | `label: u16` | \\([\ldots, c] \to [\ldots]\\) | -- | Pop \\(c\\). If \\(c \neq 0\\), seek to `jump_table[label].start`; otherwise fall through. |
| `0x04` | `NEXT` | -- | \\([\ldots] \to [\ldots]\\) | -- | Advance the active loop frame. For Range: increment current; if \\(\text{current} < \text{end}\\), seek to body start, else pop frame. For Iter: increment index; if \\(\text{index} < \text{len}\\), seek to body start, else pop frame. Errors if no loop frame is active. |
| `0x05` | `LVAL` | `reg: Register` | \\([\ldots] \to [\ldots]\\) | `write` | Copy the current loop value into `reg`. For Range: \\(\text{reg} \leftarrow \text{Int}(\text{current})\\). For Iter: \\(\text{reg} \leftarrow \text{vec}[\text{index}]\\). |
| `0x06` | `RANGE` | -- | \\([\ldots, s, n] \to [\ldots]\\) | -- | Pop \\(n\\) (count), then \\(s\\) (start). Push a Range loop frame with \\(\text{current} = s,\; \text{end} = s + n\\). |
| `0x07` | `ITER` | `reg: Register` | \\([\ldots] \to [\ldots]\\) | `read` | Validate that `reg` holds `VecInt` or `VecXqmx`; push an Iter loop frame with \\(\text{index} = 0\\). |
| `0x08` | `LIDX` | `reg: Register` | \\([\ldots] \to [\ldots]\\) | `write` | Copy the current loop *index* into `reg` as `Int`. For Range: \\(\text{reg} \leftarrow \text{Int}(\text{current})\\) (equivalent to `LVAL` because Range values are already indices). For Iter: \\(\text{reg} \leftarrow \text{Int}(\text{position in the iterated vec})\\). Errors with `NoActiveLoop` if no loop frame is active. |
| `0x09` | `HALT` | -- | \\([\ldots] \to [\ldots]\\) | -- | Stop execution immediately. |

## Branching

`JUMP` and `JUMPI` use label indices, not raw byte offsets. The label index is a
`u16` value that maps to a byte range via the program's jump table. At the
assembly level, labels are written as `.N` (e.g. `.0`, `.1`); the assembler
resolves them to indices automatically.

`TARGET` must appear at every label destination. It is a no-op at runtime but
serves as a validation marker -- the VM verifies that jump targets land on
`TARGET` instructions.

## Looping

XQVM provides two loop primitives:

### Range Loops

`RANGE` pops \\(n\\) and \\(s\\) from the stack and creates a loop frame that
iterates `current` from \\(s\\) to \\(s + n - 1\\). Use `LVAL` inside the
loop body to copy the current value into a register, and `NEXT` to advance:

```asm
PUSH 0       ; start
PUSH 10      ; count
RANGE
  LVAL r0    ; r0 = current iteration value (0, 1, ..., 9)
  ; ... loop body ...
NEXT
```

### Iterator Loops

`ITER` takes a register holding a `VecInt` or `VecXqmx` and iterates over its
elements:

```asm
ITER r1      ; r1 must hold a VecInt or VecXqmx
  LVAL r2    ; r2 = current element
  ; ... loop body ...
NEXT
```

Loops can be nested. Each `RANGE` or `ITER` pushes a frame onto the loop stack;
`NEXT` pops the frame when the loop completes.

### Loop Index vs. Loop Value

`LVAL` reads the current loop *value*: the integer being iterated for `RANGE`
loops, or the actual vec element for `ITER` loops. `LIDX` reads the current
loop *index* into the iteration source instead. The two opcodes have
overlapping but distinct semantics:

| Loop kind | `LVAL` | `LIDX` |
|-----------|--------|--------|
| `RANGE` | `Int(current)` | `Int(current)` -- identical to `LVAL`, because the values *are* indices |
| `ITER`  | the vec element at the current index (`Int` or `Model`) | `Int(position in the iterated vec)` |

Use `LIDX` inside an `ITER` loop when you need to know *where* the current
element lives in the source vec -- typically for index-based lookups or
constraint generation:

```asm
ITER r1
  LIDX r2    ; r2 = position in r1 (0, 1, 2, ...)
  LVAL r3    ; r3 = element value at that position
  ; ... loop body uses both r2 and r3 ...
NEXT
```

Calling either `LIDX` or `LVAL` outside any active loop produces a
`NoActiveLoop` runtime error.
