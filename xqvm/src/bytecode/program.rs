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

//! Complete XQVM program: just the instruction stream.
//!
//! After QUI-405 the wire format is exactly the instruction stream -- the
//! jump table is no longer serialised and is reconstructed by scanning for
//! `TARGET` opcodes at load time. [`Program::from_bytes`] takes raw bytes
//! and pre-computes the [`JumpTable`] once so subsequent VM runs do not pay
//! the scan cost.
//!
//! # Examples
//!
//! ```rust
//! use xqvm::Program;
//!
//! let program = Program::new(vec![0xFFu8]); // HALT
//! let bytes = program.encode();
//! let decoded = Program::decode(&bytes).expect("decode");
//! assert_eq!(decoded.code(), &[0xFF]);
//! assert!(decoded.jump_table().is_empty());
//! ```

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use thiserror::Error;

use super::jump_table::JumpTable;

// ---------------------------------------------------------------------------
// Program
// ---------------------------------------------------------------------------

/// A complete XQVM program.
///
/// Holds the instruction-stream bytes and a pre-computed [`JumpTable`] (the
/// table is rebuilt every time you go through [`Self::new`] or
/// [`Self::decode`]). The wire format -- what [`Self::encode`] returns -- is
/// just the raw instruction bytes, with no jump-table header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    code: Vec<u8>,
    jump_table: JumpTable,
}

impl Program {
    /// Wrap raw instruction bytes in a [`Program`], computing the jump table
    /// by scanning the buffer for `TARGET` opcodes.
    pub fn new(code: Vec<u8>) -> Self {
        let jump_table = JumpTable::scan(&code);
        Self { code, jump_table }
    }

    /// Wrap raw instruction bytes with an explicit jump table.
    ///
    /// Useful in tests where the caller has already computed the table or
    /// wants to override the scan result. In normal use [`Self::new`] is
    /// preferable.
    pub fn from_parts(code: Vec<u8>, jump_table: JumpTable) -> Self {
        Self { code, jump_table }
    }

    /// The pre-computed jump table.
    pub fn jump_table(&self) -> &JumpTable {
        &self.jump_table
    }

    /// The raw instruction bytes.
    pub fn code(&self) -> &[u8] {
        &self.code
    }

    /// Encode the program to its wire format -- the raw instruction stream.
    ///
    /// Equivalent to cloning [`Self::code`].
    pub fn encode(&self) -> Vec<u8> {
        self.code.clone()
    }

    /// Decode a program from its wire format. Cannot fail because the wire
    /// format is the raw instruction stream itself; the `Result` return is
    /// kept for API symmetry with the eventual macro-codec migration.
    ///
    /// # Errors
    ///
    /// This function currently never returns an error. The signature stays
    /// `Result` so future format additions (length headers, magic bytes)
    /// can introduce decoding failures without a breaking API change.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProgramDecodeError> {
        Ok(Self::new(bytes.to_vec()))
    }
}

/// Error returned by [`Program::decode`].
///
/// No variants today (decoding never fails for the current wire format),
/// but the type exists so future format extensions can add failures
/// without a breaking signature change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProgramDecodeError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Instruction;
    use crate::bytecode::codec;

    fn assemble(instrs: &[Instruction]) -> Vec<u8> {
        instrs.iter().flat_map(codec::encode).collect()
    }

    #[test]
    fn encode_is_just_the_code() {
        let prog = Program::new(vec![0xFF]);
        assert_eq!(prog.encode(), vec![0xFF]);
    }

    #[test]
    fn decode_round_trips() {
        let prog = Program::new(vec![0xFF, 0x00]);
        let decoded = Program::decode(&prog.encode()).unwrap();
        assert_eq!(decoded.code(), &[0xFF, 0x00]);
    }

    #[test]
    fn empty_program_has_empty_jump_table() {
        let prog = Program::new(vec![]);
        assert!(prog.jump_table().is_empty());
        assert!(prog.encode().is_empty());
    }

    #[test]
    fn jump_table_is_built_from_targets_in_code() {
        let buf = assemble(&[
            Instruction::Target {},
            Instruction::Nop {},
            Instruction::Target {},
            Instruction::Halt {},
        ]);
        let prog = Program::new(buf);
        let table = prog.jump_table();
        assert_eq!(table.len(), 2);
        assert_eq!(table.get(0), Some(0));
        assert_eq!(table.get(1), Some(2));
    }
}
