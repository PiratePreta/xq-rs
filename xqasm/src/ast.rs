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

//! Abstract syntax tree types produced by the parser.
//!
//! The parser converts raw pest pairs into these structured types before the
//! assembler processes them.  No semantic validation (unknown mnemonics, wrong
//! operand counts, etc.) is performed at this stage.
//!
//! # Examples
//!
//! ```rust
//! use xqasm::{AsmLine, Operand, ParsedInstr};
//!
//! let line = AsmLine::Instruction(ParsedInstr {
//!     mnemonic: "PUSH".to_string(),
//!     operands: vec![Operand::Integer(42)],
//!     offset: 0,
//! });
//! if let AsmLine::Instruction(instr) = &line {
//!     assert_eq!(instr.mnemonic, "PUSH");
//! }
//! ```

// ---------------------------------------------------------------------------
// Operand
// ---------------------------------------------------------------------------

/// A single instruction operand after parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operand {
    /// A register reference, e.g. `r3`.  The inner value is the slot index.
    Register(u8),
    /// A signed integer literal, e.g. `42` or `-99` or `0xFF`.
    Integer(i64),
    /// A numeric label reference, e.g. `.0`, `.1`.
    ///
    /// Only valid as the operand of `JUMP` or `JUMPI` instructions; the
    /// assembler resolves the index to a label slot in the jump table.
    LabelRef(u16),
}

// ---------------------------------------------------------------------------
// ParsedInstr
// ---------------------------------------------------------------------------

/// A fully parsed instruction with its source location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedInstr {
    /// Uppercase mnemonic string, e.g. `"PUSH"`.
    pub mnemonic: String,
    /// Operands in declaration order.
    pub operands: Vec<Operand>,
    /// Byte offset of the mnemonic within the source text.
    pub offset: usize,
}

// ---------------------------------------------------------------------------
// AsmLine
// ---------------------------------------------------------------------------

/// A single logical line of assembly source after parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsmLine {
    /// A label definition, e.g. `.0:`.
    ///
    /// The label is anchored to the byte offset of the *next* instruction.
    LabelDef {
        /// The numeric label index.
        label: u16,
        /// Byte offset of the label token within the source text.
        offset: usize,
    },
    /// An instruction with zero or more operands.
    Instruction(ParsedInstr),
}
