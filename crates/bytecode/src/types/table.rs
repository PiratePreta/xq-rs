// Copyright (C) 2026 Postquant Labs Incorporated
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later

/// Invoke `$mac!` with the complete XQVM opcode table.
///
/// The callback macro receives the full comma-separated list of 84 opcode
/// entries. Each entry has the form:
///
/// ```text
/// (code, Variant, "MNEMONIC", "doc", {field_name: FieldType, ...})
/// ```
///
/// | Position | Type | Description |
/// |---|---|---|
/// | `code` | `u8` literal | Wire-encoding byte |
/// | `Variant` | `ident` | `PascalCase` Rust enum variant name |
/// | `"MNEMONIC"` | `str` literal | Uppercase assembly mnemonic |
/// | `"doc"` | `str` literal | Single-sentence description |
/// | `{field: Type, ...}` | named-field list | Zero or more named operand fields |
///
/// Codes `0x08` and `0x0D` are unassigned gaps reserved for future use;
/// the decoder and VM treat them as illegal opcodes.
///
/// # Examples
///
/// ```rust
/// macro_rules! collect_mnemonics {
///     ( $( ($code:literal, $var:ident, $mnem:literal, $doc:literal, {$($f:tt)*}) ),* $(,)? ) => {
///         &[ $( $mnem ),* ]
///     }
/// }
///
/// let mnemonics: &[&str] = aglais_xqvm_bytecode::opcodes!(collect_mnemonics);
/// assert!(mnemonics.contains(&"HALT"));
/// assert!(mnemonics.contains(&"ENERGY"));
/// ```
#[macro_export]
macro_rules! opcodes {
    ($mac:ident) => {
        $mac! {
            // ---------------------------------------------------------------
            // Control Flow
            // ---------------------------------------------------------------
            (0x00, Nop,     "NOP",      "No operation.",
             {}),
            (0x01, Target,  "TARGET",   "Mark a valid jump destination.",
             {}),
            (0x02, Jump,    "JUMP",     "Unconditionally jump to a basic block by label index.",
             {label: u16}),
            (0x03, JumpI,   "JUMPI",    "Jump to a basic block by label index if the top of the stack is non-zero.",
             {label: u16}),
            (0x04, Next,    "NEXT",     "Advance the loop index; jump back or exit the current loop.",
             {}),
            (0x05, LVal,    "LVAL",     "Copy the current loop value into a register.",
             {reg: $crate::Register}),
            (0x06, Range,   "RANGE",    "Start a range loop over [start, start + count).",
             {}),
            (0x07, Iter,    "ITER",     "Start a vec iteration over a slice of a register's vec.",
             {reg: $crate::Register}),
            // 0x08 is reserved (unassigned gap).
            (0x09, Halt,    "HALT",     "Stop execution.",
             {}),
            // ---------------------------------------------------------------
            // Register I/O
            // ---------------------------------------------------------------
            (0x0A, Load,    "LOAD",     "Push the value of an int register onto the stack.",
             {reg: $crate::Register}),
            (0x0B, Stow,    "STOW",     "Pop the top of the stack into an int register.",
             {reg: $crate::Register}),
            (0x0C, Drop,    "DROP",     "Reset a register to Int(0).",
             {reg: $crate::Register}),
            // 0x0D is reserved (unassigned gap).
            (0x0E, Input,   "INPUT",    "Pop a calldata slot index and load that slot into a register.",
             {reg: $crate::Register}),
            (0x0F, Output,  "OUTPUT",   "Pop an output slot index and write the register to it.",
             {reg: $crate::Register}),
            // ---------------------------------------------------------------
            // Stack Manipulation
            // ---------------------------------------------------------------
            (0x10, Pop,     "POP",      "Discard the top of the stack.",
             {}),
            (0x11, Push1,   "PUSH1",    "Push a 1-byte big-endian signed constant, sign-extended to i64.",
             {val: [u8; 1]}),
            (0x12, Push2,   "PUSH2",    "Push a 2-byte big-endian signed constant, sign-extended to i64.",
             {val: [u8; 2]}),
            (0x13, Push3,   "PUSH3",    "Push a 3-byte big-endian signed constant, sign-extended to i64.",
             {val: [u8; 3]}),
            (0x14, Push4,   "PUSH4",    "Push a 4-byte big-endian signed constant, sign-extended to i64.",
             {val: [u8; 4]}),
            (0x15, Push5,   "PUSH5",    "Push a 5-byte big-endian signed constant, sign-extended to i64.",
             {val: [u8; 5]}),
            (0x16, Push6,   "PUSH6",    "Push a 6-byte big-endian signed constant, sign-extended to i64.",
             {val: [u8; 6]}),
            (0x17, Push7,   "PUSH7",    "Push a 7-byte big-endian signed constant, sign-extended to i64.",
             {val: [u8; 7]}),
            (0x18, Push8,   "PUSH8",    "Push a full 8-byte big-endian signed constant (i64).",
             {val: [u8; 8]}),
            // 0x19 is a gap.
            (0x1A, Sclr,    "SCLR",     "Clear the entire value stack.",
             {}),
            (0x1B, Swap,    "SWAP",     "Swap the top two stack elements.",
             {}),
            (0x1C, Copy,    "COPY",     "Duplicate the top of the stack.",
             {}),
            // ---------------------------------------------------------------
            // Arithmetic
            // ---------------------------------------------------------------
            (0x20, Add,     "ADD",      "Pop b and a; push a + b.",
             {}),
            (0x21, Sub,     "SUB",      "Pop b and a; push a - b.",
             {}),
            (0x22, Mul,     "MUL",      "Pop b and a; push a * b.",
             {}),
            (0x23, Div,     "DIV",      "Pop b and a; push a / b (truncating integer division).",
             {}),
            (0x24, Modulo,  "MOD",      "Pop b and a; push a % b.",
             {}),
            (0x25, Sqr,     "SQR",      "Pop a; push a * a.",
             {}),
            (0x26, Abs,     "ABS",      "Pop a; push |a|.",
             {}),
            (0x27, Neg,     "NEG",      "Pop a; push -a.",
             {}),
            (0x28, Min,     "MIN",      "Pop b and a; push min(a, b).",
             {}),
            (0x29, Max,     "MAX",      "Pop b and a; push max(a, b).",
             {}),
            (0x2A, Inc,     "INC",      "Pop a; push a + 1.",
             {}),
            (0x2B, Dec,     "DEC",      "Pop a; push a - 1.",
             {}),
            // ---------------------------------------------------------------
            // Comparison  (result: 1 if true, 0 if false)
            // ---------------------------------------------------------------
            (0x30, Eq,      "EQ",       "Pop b and a; push 1 if a == b, else 0.",
             {}),
            (0x31, Lt,      "LT",       "Pop b and a; push 1 if a < b, else 0.",
             {}),
            (0x32, Gt,      "GT",       "Pop b and a; push 1 if a > b, else 0.",
             {}),
            (0x33, Lte,     "LTE",      "Pop b and a; push 1 if a <= b, else 0.",
             {}),
            (0x34, Gte,     "GTE",      "Pop b and a; push 1 if a >= b, else 0.",
             {}),
            // ---------------------------------------------------------------
            // Logical Boolean
            // ---------------------------------------------------------------
            (0x36, Not,     "NOT",      "Pop a; push 1 if a == 0, else 0.",
             {}),
            (0x37, And,     "AND",      "Pop b and a; push 1 if both are non-zero, else 0.",
             {}),
            (0x38, Or,      "OR",       "Pop b and a; push 1 if either is non-zero, else 0.",
             {}),
            (0x39, Xor,     "XOR",      "Pop b and a; push 1 if exactly one is non-zero, else 0.",
             {}),
            // ---------------------------------------------------------------
            // Bitwise
            // ---------------------------------------------------------------
            (0x3A, BAnd,    "BAND",     "Pop b and a; push a & b.",
             {}),
            (0x3B, BOr,     "BOR",      "Pop b and a; push a | b.",
             {}),
            (0x3C, BXor,    "BXOR",     "Pop b and a; push a ^ b.",
             {}),
            (0x3D, BNot,    "BNOT",     "Pop a; push ~a.",
             {}),
            (0x3E, Shl,     "SHL",      "Pop b and a; push a << b.",
             {}),
            (0x3F, Shr,     "SHR",      "Pop b and a; push a >> b (arithmetic right shift, sign-preserving).",
             {}),
            // ---------------------------------------------------------------
            // Allocators
            // ---------------------------------------------------------------
            (0x40, Bqmx,    "BQMX",     "Pop size; allocate a binary QUBO model ([0, 1] domain) into a register.",
             {reg: $crate::Register}),
            (0x41, Sqmx,    "SQMX",     "Pop size; allocate a spin Ising model ([-1, 1] domain) into a register.",
             {reg: $crate::Register}),
            (0x42, Xqmx,    "XQMX",     "Pop k then size; allocate a discrete model with signed centered domain [-k, k-1] into a register. Errors when k < 2.",
             {reg: $crate::Register}),
            (0x43, Bsmx,    "BSMX",     "Pop size; allocate a binary sample ([0, 1] domain) into a register.",
             {reg: $crate::Register}),
            (0x44, Ssmx,    "SSMX",     "Pop size; allocate a spin sample ([-1, 1] domain) into a register.",
             {reg: $crate::Register}),
            (0x45, Xsmx,    "XSMX",     "Pop k then size; allocate a discrete sample with signed centered domain [-k, k-1] into a register. Errors when k < 2.",
             {reg: $crate::Register}),
            // ---------------------------------------------------------------
            // Vec Allocators
            // ---------------------------------------------------------------
            (0x4A, Vec,     "VEC",      "Create an empty vec (element type inferred on first push) in a register.",
             {reg: $crate::Register}),
            (0x4B, VecI,    "VECI",     "Create an empty `vec<int>` in a register.",
             {reg: $crate::Register}),
            (0x4C, VecX,    "VECX",     "Create an empty `vec<xqmx>` in a register.",
             {reg: $crate::Register}),
            // ---------------------------------------------------------------
            // Vector Access
            // ---------------------------------------------------------------
            (0x50, VecPush, "VECPUSH",  "Pop a value; append it to the register's vec.",
             {reg: $crate::Register}),
            (0x51, VecGet,  "VECGET",   "Pop index; push `vec[index]` from the register's vec.",
             {reg: $crate::Register}),
            (0x52, VecSet,  "VECSET",   "Pop value and index; set `vec[index]` in the register's vec.",
             {reg: $crate::Register}),
            (0x53, VecLen,  "VECLEN",   "Push the length of the register's vec onto the stack.",
             {reg: $crate::Register}),
            // ---------------------------------------------------------------
            // Index Math
            // ---------------------------------------------------------------
            (0x5A, IdxGrid, "IDXGRID",  "Pop cols, col, row; push the flat grid index row * cols + col.",
             {}),
            (0x5B, IdxTriu, "IDXTRIU",  "Pop j and i (i <= j); push the upper-triangular index for (i, j).",
             {}),
            // ---------------------------------------------------------------
            // XQMX Coefficient Access
            // ---------------------------------------------------------------
            (0x60, GetLine, "GETLINE",  "Pop i; push `linear[i]` from the register's model (0 if absent).",
             {reg: $crate::Register}),
            (0x61, SetLine, "SETLINE",  "Pop value and i; set `linear[i]` in the register's model.",
             {reg: $crate::Register}),
            (0x62, AddLine, "ADDLINE",  "Pop delta and i; add delta to `linear[i]` in the register's model.",
             {reg: $crate::Register}),
            (0x63, GetQuad, "GETQUAD",  "Pop j and i; push `quadratic[i, j]` from the register's model (0 if absent).",
             {reg: $crate::Register}),
            (0x64, SetQuad, "SETQUAD",  "Pop value, j, and i; set `quadratic[i, j]` in the register's model.",
             {reg: $crate::Register}),
            (0x65, AddQuad, "ADDQUAD",  "Pop delta, j, and i; add delta to `quadratic[i, j]` in the register's model.",
             {reg: $crate::Register}),
            // ---------------------------------------------------------------
            // XQMX Grid
            // ---------------------------------------------------------------
            (0x66, Resize,  "RESIZE",   "Pop cols and rows; set the grid dimensions of the register's model.",
             {reg: $crate::Register}),
            (0x67, RowFind, "ROWFIND",  "Pop value and row; push the first column where the value matches, or -1.",
             {reg: $crate::Register}),
            (0x68, ColFind, "COLFIND",  "Pop value and col; push the first row where the value matches, or -1.",
             {reg: $crate::Register}),
            (0x69, RowSum,  "ROWSUM",   "Pop row; push the sum of all linear values in that grid row.",
             {reg: $crate::Register}),
            (0x6A, ColSum,  "COLSUM",   "Pop col; push the sum of all linear values in that grid column.",
             {reg: $crate::Register}),
            // ---------------------------------------------------------------
            // XQMX High-Level Constraints
            // ---------------------------------------------------------------
            (0x70, OneHotR, "ONEHOTR",  "Pop penalty and row; add a one-hot constraint over the grid row.",
             {reg: $crate::Register}),
            (0x71, OneHotC, "ONEHOTC",  "Pop penalty and col; add a one-hot constraint over the grid column.",
             {reg: $crate::Register}),
            (0x72, Exclude, "EXCLUDE",  "Pop penalty, j, and i; add a mutual-exclusion constraint between variables i and j.",
             {reg: $crate::Register}),
            (0x73, Implies, "IMPLIES",  "Pop penalty, j, and i; add an implication constraint from variable i to variable j.",
             {reg: $crate::Register}),
            (0x7F, Energy,  "ENERGY",   "Compute the Hamiltonian energy of a sample against a model; push the result.",
             {model: $crate::Register, sample: $crate::Register}),
        }
    };
}
