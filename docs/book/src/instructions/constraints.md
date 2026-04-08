# High-Level Constraints

These instructions inject QUBO penalty terms for common combinatorial
constraints, expanding into linear and quadratic coefficient deltas
automatically. `reg` must hold `Model` with grid dimensions pre-set by
`RESIZE`. All coefficients are `i64`.

## `0x70` -- `ONEHOTR reg`

**Stack:** \\([\ldots, \text{row}, \text{penalty}] \to [\ldots]\\)
**Register effect:** `mutate`

Pop `penalty`, then `row`. Apply the one-hot constraint over all variables in
grid row `row`:

$$H \mathrel{+}= \text{penalty} \cdot \left(\sum_c x_{\text{row},c} - 1\right)^2$$

Expanding (binary variables: \\(x^2 = x\\)):

$$\text{linear}[\text{row} \cdot \text{cols} + c] \mathrel{+}= -\text{penalty} \qquad \forall\; c \in [0, \text{cols})$$

$$\text{quad}[\text{row} \cdot \text{cols} + c_i,\; \text{row} \cdot \text{cols} + c_j] \mathrel{+}= 2 \cdot \text{penalty} \qquad \forall\; c_i < c_j$$

## `0x71` -- `ONEHOTC reg`

**Stack:** \\([\ldots, \text{col}, \text{penalty}] \to [\ldots]\\)
**Register effect:** `mutate`

Pop `penalty`, then `col`. One-hot over all variables in grid column `col`:

$$\text{linear}[r_i \cdot \text{cols} + \text{col}] \mathrel{+}= -\text{penalty} \qquad \forall\; r_i \in [0, \text{rows})$$

$$\text{quad}[r_i \cdot \text{cols} + \text{col},\; r_j \cdot \text{cols} + \text{col}] \mathrel{+}= 2 \cdot \text{penalty} \qquad \forall\; r_i < r_j$$

## `0x72` -- `EXCLUDE reg`

**Stack:** \\([\ldots, i, j, \text{penalty}] \to [\ldots]\\)
**Register effect:** `mutate`

Pop `penalty`, then \\(j\\), then \\(i\\). Add mutual-exclusion: penalise
\\(x_i = 1\\) and \\(x_j = 1\\) simultaneously.

$$\text{quad}[i, j] \mathrel{+}= \text{penalty}$$

## `0x73` -- `IMPLIES reg`

**Stack:** \\([\ldots, i, j, \text{penalty}] \to [\ldots]\\)
**Register effect:** `mutate`

Pop `penalty`, then \\(j\\), then \\(i\\). Add implication \\(i \Rightarrow j\\):
penalise \\(x_i = 1\\) with \\(x_j = 0\\).

$$H \mathrel{+}= \text{penalty} \cdot x_i \cdot (1 - x_j) = \text{penalty} \cdot x_i - \text{penalty} \cdot x_i \cdot x_j$$

$$\text{linear}[i] \mathrel{+}= \text{penalty}$$

$$\text{quad}[i, j] \mathrel{+}= -\text{penalty}$$

## Usage Pattern

Constraint instructions are designed to work with grid models. A typical
pattern for a TSP:

```asm
; Allocate model and set grid
PUSH 16
BQMX r0
PUSH 4
PUSH 4
RESIZE r0

; Apply one-hot constraints on each row and column
PUSH 0
PUSH 4
RANGE
  LVAL r1
  LOAD r1
  PUSH 100       ; penalty weight
  ONEHOTR r0     ; each city visits exactly one position
NEXT

PUSH 0
PUSH 4
RANGE
  LVAL r1
  LOAD r1
  PUSH 100
  ONEHOTC r0     ; each position has exactly one city
NEXT
```
