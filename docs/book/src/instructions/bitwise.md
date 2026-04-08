# Bitwise

Operate on raw `i64` bit patterns.

| Code | Mnemonic | Stack Effect | Description |
|------|----------|--------------|-------------|
| `0x3A` | `BAND` | \\([\ldots, a, b] \to [\ldots, a \mathbin{\\&} b]\\) | Bitwise AND. |
| `0x3B` | `BOR` | \\([\ldots, a, b] \to [\ldots, a \mathbin{\mid} b]\\) | Bitwise OR. |
| `0x3C` | `BXOR` | \\([\ldots, a, b] \to [\ldots, a \oplus b]\\) | Bitwise XOR. |
| `0x3D` | `BNOT` | \\([\ldots, a] \to [\ldots, \mathord{\sim}a]\\) | Bitwise NOT (one's complement). |
| `0x3E` | `SHL` | \\([\ldots, a, b] \to [\ldots, a \ll b]\\) | Left shift. \\(b\\) must satisfy \\(0 \le b < 64\\); otherwise errors. |
| `0x3F` | `SHR` | \\([\ldots, a, b] \to [\ldots, a \ggg b]\\) | Logical (unsigned) right shift. \\(b\\) must satisfy \\(0 \le b < 64\\). High bit filled with \\(0\\). |

None of these instructions have register effects.

## Shift Behaviour

- `SHL` performs a signed left shift. Bits shifted out of the high end are
  discarded. The shift amount must be in \\([0, 64)\\).
- `SHR` performs a **logical** (unsigned) right shift, not an arithmetic shift.
  The high bit is always filled with \\(0\\), regardless of the sign of \\(a\\). This
  matches Rust's `(a as u64) >> b` cast behaviour.
- Both shift instructions error with `InvalidShift` if \\(b\\) is outside \\([0, 64)\\).
