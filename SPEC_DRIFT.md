# XQVM Spec Differences: xq-rs vs xq-py

`XQVM_SPEC.md` (mirroring `xq-py`) is authoritative. This file tracks every
semantic divergence between the two implementations. Resolved items are
archived below the open section.

---

## Open items

No open items. All Phase 0 divergences have been resolved in QUI-437.

---

## Recently resolved

- **Assembler jump-width selection** — `JUMP .N` / `JUMPI .N` now emit `JUMP1` /
  `JUMPI1` (u8) when the resolved sequential target ID ≤ 255; two-pass narrowing
  in `InstructionBuilder::build` step 5 (`crates/bytecode/src/builder.rs`).
  (QUI-437)
- **DIV floor division** — `exec_div` now applies the floor correction
  `q.wrapping_sub(1)` when the truncated remainder is nonzero and operands have
  opposite signs, matching Python `//` semantics (`crates/vm/src/vm.rs`).
  (QUI-437)
- **MOD divisor-sign modulo** — `exec_modulo` adds `b` to the truncated
  remainder when the remainder is nonzero and has the opposite sign from the
  divisor, matching Python `%` semantics (`crates/vm/src/vm.rs`). (QUI-437)
- **DROP / LOAD unset-register fault** — added `RegVal::Unset` variant; all
  register slots initialize to `Unset`; `exec_drop` writes `Unset`; `exec_load`
  returns `Error::UnsetRegister` on an unset slot. Matches xq-py
  `RegisterNotFound` (`crates/vm/src/value.rs`, `error.rs`, `vm.rs`). (QUI-437)
- **Opcode table** — TARGET=0x00, JUMP1/2, JUMPI1/2, LIDX, reordered
  loop/halt opcodes; count = 87 (`801b00f`, `37b0379`).
- **Jump targets** — inline `TARGET` opcode plus runtime pre-scan, replacing
  the old jump-table header (`e1d617a`, `d43791e`).
- **JUMP / JUMPI** — split into u8 and u16 variants (`79fc87a`).
- **ITER** — stack-driven slicing `vec[start_idx:end_idx]`, with `start_offset`
  preserved for LIDX (`49e46ed`).
- **Discrete domain** — XQMX / XSMX now centred on `[-k, k-1]` (`f8cecfd`).
- **SHR** — arithmetic (sign-preserving) right shift (`0e0b559`).
- **ENERGY** — rejects a Model register in the sample slot (`29934f3`).
- **Wire format** — flat instruction stream, no header, magic, or length prefix
  (`d43791e`, `cd9114e`).

---

## Representative program checklist (Phase 0 manual sign-off)

Run each program by hand through both xq-py and xq-rs; compare final stack, registers, and output slots.

| # | Program | What it exercises | Status |
|---|---------|-------------------|--------|
| 1 | `PUSH -7 \| PUSH 2 \| DIV \| HALT`  | Floor division (neg ÷ pos) | ✅ automated |
| 2 | `PUSH 7 \| PUSH -2 \| DIV \| HALT`  | Floor division (pos ÷ neg) | ✅ automated |
| 3 | `PUSH -7 \| PUSH 2 \| MOD \| HALT`  | Divisor-sign modulo        | ✅ automated |
| 4 | `PUSH 7 \| PUSH -2 \| MOD \| HALT`  | Divisor-sign modulo        | ✅ automated |
| 5 | `PUSH 1 \| STOW r0 \| DROP r0 \| LOAD r0 \| HALT` | DROP unsets register → LOAD faults | ✅ automated |
| 6 | `LOAD r0 \| HALT`                   | LOAD on never-set register faults  | ✅ automated |
| 7 | `PUSH 3 \| PUSH 0 \| JUMP .0 \| .0: \| HALT` | u8 jump (narrowed to JUMP1)        | ✅ automated |
| 8 | RANGE loop: `PUSH 0 \| PUSH 5 \| RANGE \| .0: \| LVAL r0 \| NEXT .0 \| HALT` | Loop with LVAL | [ ] |
| 9 | ITER loop: `VECI r1 \| ... \| PUSH 0 \| PUSH 3 \| ITER r1 \| .0: \| LVAL r2 \| LIDX r3 \| NEXT .0 \| HALT` | ITER with LIDX offset | [ ] |
| 10 | `PUSH 4 \| BQMX r0 \| ... set coeffs ... \| BSMX r1 \| ... \| ENERGY r0 r1 \| HALT` | ENERGY basic smoke-test | [ ] |
