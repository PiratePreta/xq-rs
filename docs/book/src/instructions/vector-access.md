# Vector Access

Instructions for reading, writing, and querying register-held vectors.

| Code | Mnemonic | Arguments | Stack Effect | Register Effect | Description |
|------|----------|-----------|--------------|-----------------|-------------|
| `0x50` | `VECPUSH` | `reg: Register` | \\([\ldots, v] \to [\ldots]\\) | `mutate` | Pop \\(v\\). Append \\(v\\) to `reg`'s `VecInt`. |
| `0x51` | `VECGET` | `reg: Register` | \\([\ldots, i] \to [\ldots, v]\\) | `read` | Pop \\(i\\). Bounds-check: \\(0 \le i < \text{len}\\). Push \\(\text{vec}[i]\\). |
| `0x52` | `VECSET` | `reg: Register` | \\([\ldots, i, v] \to [\ldots]\\) | `mutate` | Pop \\(v\\), then \\(i\\). Bounds-check: \\(0 \le i < \text{len}\\). Set \\(\text{vec}[i] \leftarrow v\\). |
| `0x53` | `VECLEN` | `reg: Register` | \\([\ldots] \to [\ldots, n]\\) | `read` | `reg` must hold `VecInt` or `VecXqmx`. Push \\(\lvert\text{vec}\rvert\\) as `i64`. |

## Type Requirements

- `VECPUSH`, `VECGET`, and `VECSET` require the register to hold `VecInt`.
- `VECLEN` accepts both `VecInt` and `VecXqmx`.
- All indexing operations perform bounds checking and error with
  `IndexOutOfBounds` on violation.

## Example

```asm
VEC r0          ; r0 = empty VecInt
PUSH 10
VECPUSH r0      ; r0 = [10]
PUSH 20
VECPUSH r0      ; r0 = [10, 20]
PUSH 0
VECGET r0       ; stack = [..., 10]
```
