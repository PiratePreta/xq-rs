# Loop Stack

The loop stack manages `RANGE` and `ITER` loop state. Each active loop pushes a
frame; `NEXT` either advances the loop or pops the frame when iteration
completes.

## Loop Frames

Each frame records:

- **Kind** -- `Range` or `Iter`.
- **`body_start`** -- byte offset of the first instruction after `RANGE`/`ITER`.
  This is where `NEXT` seeks back to on each iteration.

### Range Loops

```
LoopKind::Range {
    current: i64,    // current iteration value
    end: i64,        // exclusive upper bound (start + count)
}
```

`RANGE` pops `count` and `start` from the stack. The loop iterates `current`
from `start` to `end - 1` (where `end = start + count`, wrapping). On each
`NEXT`, `current` is incremented. If `current < end`, execution seeks back to
`body_start`; otherwise the frame is popped and execution falls through.

```asm
PUSH 5       ; start = 5
PUSH 3       ; count = 3
RANGE        ; iterates current = 5, 6, 7
  LVAL r0    ; r0 ← Int(current)
  ; ... body ...
NEXT
```

### Iterator Loops

```
LoopKind::Iter {
    reg: Register,   // register holding the vector
    index: usize,    // current element index
}
```

`ITER reg` validates that `reg` holds `VecInt` or `VecXqmx`, then pushes a
frame with `index = 0`. On each `NEXT`, `index` is incremented. If
`index < len(reg)`, execution seeks back; otherwise the frame is popped.

```asm
; Assume r1 holds VecInt([10, 20, 30])
ITER r1
  LVAL r2    ; r2 ← vec[index] (10, 20, 30 in turn)
  ; ... body ...
NEXT
```

## LVAL -- Reading the Loop Value

`LVAL reg` copies the current loop value into a register:

- **Range:** `reg ← Int(current)`
- **Iter over VecInt:** `reg ← Int(vec[index])`
- **Iter over VecXqmx:** `reg ← Model(vec[index])`

The element type is preserved: iterating over a `VecXqmx` yields `Model`
values, not integers.

## Nesting

Loops can be nested to arbitrary depth. Each `RANGE` or `ITER` pushes a new
frame. `LVAL` and `NEXT` always operate on the **innermost** (most recently
pushed) frame.

```asm
PUSH 0
PUSH 3
RANGE              ; outer loop: 0, 1, 2
  LVAL r0
  PUSH 0
  PUSH 4
  RANGE            ; inner loop: 0, 1, 2, 3
    LVAL r1
    ; r0 = outer value, r1 = inner value
  NEXT
NEXT
```

## Errors

- **`NoActiveLoop`** -- `NEXT` or `LVAL` with an empty loop stack.
- **`RegisterType`** -- `ITER` on a register that is not `VecInt` or `VecXqmx`.
