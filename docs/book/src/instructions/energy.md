# Energy Evaluation

## `0x7F` -- `ENERGY model sample`

**Stack:** \\([\ldots] \to [\ldots, E]\\)
**Register effect:** `read` -- both `model` and `sample` are read-only

This is the only instruction with two register operands.

The `model` register must hold `Model`. The `sample` register may hold either:

- **`Sample(s)`** -- `s.values` is used directly as the variable assignment
  vector.
- **`Model(s)`** -- \\(\text{linear}[i]\\) of the sample model is used as \\(x_i\\) for each
  \\(i \in [0, \text{size})\\). This allows a model built via `SETLINE`/`ADDLINE` to act as
  a sample.

## Hamiltonian

Evaluates the quadratic Hamiltonian:

$$E = \sum_{i} \text{linear}[i] \cdot x_i \;+\; \sum_{i < j} \text{quad}[i,j] \cdot x_i \cdot x_j$$

The result is pushed as `i64`. Arithmetic uses wrapping semantics on overflow.

## Errors

- **`SizeMismatch`** -- if \\(\lvert\text{sample}\rvert \neq \text{model.size}\\).

## Example

```asm
; Create a 2-variable binary model
PUSH 2
BQMX r0

; Set linear[0] = 3, linear[1] = -2
PUSH 0
PUSH 3
SETLINE r0
PUSH 1
PUSH -2
SETLINE r0

; Set quad[0,1] = 5
PUSH 0
PUSH 1
PUSH 5
SETQUAD r0

; Create a sample with x = [1, 1]
PUSH 2
BSMX r1
PUSH 0
PUSH 1
VECSET r1       ; sample[0] = 1
PUSH 1
PUSH 1
VECSET r1       ; sample[1] = 1

; Evaluate: E = 3·1 + (-2)·1 + 5·1·1 = 6
ENERGY r0 r1
HALT
```

In this example, the energy evaluates to:

$$E = 3 \cdot 1 + (-2) \cdot 1 + 5 \cdot 1 \cdot 1 = 6$$
