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

//! Complete XQVM program: raw encoded instruction bytes.
//!
//! A [`Program`] is the top-level unit of XQVM bytecode. It wraps the raw
//! encoded instruction stream and provides [`encode`](Program::encode) /
//! [`decode`](Program::decode) for the binary wire format.
//!
//! ## Wire format
//!
//! The binary wire format is the instruction byte stream directly -- no pool
//! header, no length prefix. [`encode`](Program::encode) returns the code
//! bytes as-is; [`decode`](Program::decode) wraps an incoming slice.
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_bytecode::Program;
//!
//! let program = Program::new(vec![0x0Fu8]); // HALT
//! let bytes = program.encode();
//! let decoded = Program::decode(&bytes);
//! assert_eq!(decoded.code(), &[0x0F]);
//! ```

// ---------------------------------------------------------------------------
// Program
// ---------------------------------------------------------------------------

/// A complete XQVM program: raw encoded instruction bytes.
///
/// The binary wire format is the instruction byte stream directly --
/// no pool header, no length prefix. Use [`encode`](Self::encode) and
/// [`decode`](Self::decode) for serialisation.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_bytecode::Program;
///
/// let program = Program::new(vec![0x0Fu8]); // HALT
/// let bytes = program.encode();
/// let decoded = Program::decode(&bytes);
/// assert_eq!(decoded.code(), &[0x0F]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    code: Vec<u8>,
}

impl Program {
    /// Create a new program from raw instruction bytes.
    pub fn new(code: Vec<u8>) -> Self {
        Self { code }
    }

    /// The raw instruction bytes.
    pub fn code(&self) -> &[u8] {
        &self.code
    }

    /// Encode the program to a byte buffer (identity: returns the code bytes).
    pub fn encode(&self) -> Vec<u8> {
        self.code.clone()
    }

    /// Decode a program from its encoded byte representation.
    pub fn decode(bytes: &[u8]) -> Self {
        Self::new(bytes.to_vec())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip() {
        let prog = Program::new(vec![0x0F]);
        assert_eq!(Program::decode(&prog.encode()).code(), prog.code());
    }

    #[test]
    fn empty_program() {
        let prog = Program::new(vec![]);
        assert_eq!(prog.encode(), b"");
    }

    #[test]
    fn code_is_preserved() {
        let prog = Program::new(vec![0x0F, 0x00]);
        let decoded = Program::decode(&prog.encode());
        assert_eq!(decoded.code(), &[0x0F, 0x00]);
    }
}
