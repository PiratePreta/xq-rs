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

//! Complete XQVM program: jump table + instruction bytes.
//!
//! A [`Program`] is the top-level unit of XQVM bytecode. It wraps a
//! [`JumpTable`] and the raw instruction stream, and provides
//! [`encode`](Program::encode) / [`decode`](Program::decode) for the
//! binary wire format.
//!
//! ## Wire format
//!
//! The binary format is: `jump_table_bytes ++ code_bytes`.
//! [`encode`](Program::encode) serialises the jump table header followed by
//! the instruction bytes. [`decode`](Program::decode) parses the jump table
//! first, then treats the remainder as code.
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_bytecode::Program;
//!
//! let program = Program::new(vec![0x0Fu8]); // HALT, empty jump table
//! let bytes = program.encode();
//! let decoded = Program::decode(&bytes).unwrap();
//! assert_eq!(decoded.code(), &[0x0F]);
//! ```

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::jump_table::{DecodeError, JumpTable};

// ---------------------------------------------------------------------------
// Program
// ---------------------------------------------------------------------------

/// A complete XQVM program: jump table + instruction bytes.
///
/// Use [`encode`](Self::encode) and [`decode`](Self::decode) for
/// serialisation. The jump table maps label indices used by `JUMP`/`JUMPI`
/// to basic-block byte ranges in the instruction stream.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_bytecode::{Program, JumpTable, JumpEntry};
///
/// let table = JumpTable::new(vec![
///     JumpEntry { label: 0, start: 0, end: 3 },
/// ]);
/// let prog = Program::new_with_table(table, vec![0x00, 0x00, 0x0F]);
/// assert_eq!(prog.jump_table().len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    jump_table: JumpTable,
    code: Vec<u8>,
}

impl Program {
    /// Create a program from raw instruction bytes (empty jump table).
    pub fn new(code: Vec<u8>) -> Self {
        Self {
            jump_table: JumpTable::default(),
            code,
        }
    }

    /// Create a program with an explicit jump table.
    pub fn new_with_table(jump_table: JumpTable, code: Vec<u8>) -> Self {
        Self { jump_table, code }
    }

    /// The jump table.
    pub fn jump_table(&self) -> &JumpTable {
        &self.jump_table
    }

    /// The raw instruction bytes.
    pub fn code(&self) -> &[u8] {
        &self.code
    }

    /// Encode the program to a byte buffer (jump table + code).
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = self.jump_table.encode();
        buf.extend_from_slice(&self.code);
        buf
    }

    /// Decode a program from its encoded byte representation.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] if the jump table header is malformed or
    /// truncated.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let (jump_table, consumed) = JumpTable::decode(bytes)?;
        let code = bytes.get(consumed..).unwrap_or_default().to_vec();
        Ok(Self { jump_table, code })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jump_table::JumpEntry;

    #[test]
    fn encode_decode_roundtrip() {
        let prog = Program::new(vec![0x0F]);
        assert_eq!(Program::decode(&prog.encode()).unwrap().code(), prog.code());
    }

    #[test]
    fn empty_program() {
        let prog = Program::new(vec![]);
        let bytes = prog.encode();
        // Empty table header (2 bytes) + no code
        assert_eq!(bytes, [0x00, 0x00]);
    }

    #[test]
    fn code_is_preserved() {
        let prog = Program::new(vec![0x0F, 0x00]);
        let decoded = Program::decode(&prog.encode()).unwrap();
        assert_eq!(decoded.code(), &[0x0F, 0x00]);
    }

    #[test]
    fn roundtrip_with_jump_table() {
        let table = JumpTable::new(vec![
            JumpEntry {
                label: 0,
                start: 0,
                end: 3,
            },
            JumpEntry {
                label: 1,
                start: 3,
                end: 5,
            },
        ]);
        let prog = Program::new_with_table(table, vec![0x00, 0x00, 0x0F, 0x00, 0x0F]);
        let decoded = Program::decode(&prog.encode()).unwrap();
        assert_eq!(decoded.jump_table(), prog.jump_table());
        assert_eq!(decoded.code(), prog.code());
    }
}
