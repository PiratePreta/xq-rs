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

//! Complete XQVM program: a constant pool combined with instruction bytes.
//!
//! A [`Program`] is the top-level unit of XQVM bytecode. It bundles a
//! [`ConstantPool`] with the raw encoded instruction stream and provides
//! [`encode`](Program::encode) / [`decode`](Program::decode) for the binary
//! wire format.
//!
//! ## Wire format
//!
//! ```text
//! [pool_count: u16 BE]
//! [entry_0:    i64 BE]  -- repeated pool_count times
//! ...
//! [instruction bytes]   -- remainder of the slice
//! ```
//!
//! An empty pool is encoded as `[0x00, 0x00]` followed immediately by the
//! instruction bytes. The instruction bytes are not parsed here -- that is
//! done lazily by [`InstructionStream`](crate::stream::InstructionStream).
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_bytecode::pool::ConstantPool;
//! use aglais_xqvm_bytecode::program::Program;
//!
//! let mut pool = ConstantPool::new();
//! let _idx = pool.intern(1_000_000_000i64).unwrap();
//! let code = vec![0x0Fu8]; // HALT
//!
//! let program = Program::new(pool, code);
//! let bytes = program.encode();
//!
//! let decoded = Program::decode(&bytes).unwrap();
//! assert_eq!(decoded.pool().get(0), Some(1_000_000_000i64));
//! assert_eq!(decoded.code(), &[0x0F]);
//! ```

use thiserror::Error;

use crate::pool::ConstantPool;

// ---------------------------------------------------------------------------
// DecodeError
// ---------------------------------------------------------------------------

/// Error returned when decoding a [`Program`] from a malformed byte slice.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeError {
    /// The byte slice is too short to contain the pool header.
    ///
    /// Either the 2-byte pool count itself is missing, or the slice does not
    /// contain enough bytes for all pool entries declared by the count field.
    #[error(
        "bytecode too short to read pool header: \
         expected {required} bytes, found {available}"
    )]
    PoolTruncated {
        /// Minimum number of bytes needed to decode the pool header.
        required: usize,
        /// Actual number of bytes in the input.
        available: usize,
    },
}

// ---------------------------------------------------------------------------
// Program
// ---------------------------------------------------------------------------

/// A complete XQVM program: a constant pool plus encoded instruction bytes.
///
/// [`encode`](Self::encode) serialises the program to a flat byte buffer.
/// [`decode`](Self::decode) parses that buffer back, splitting the pool header
/// from the instruction stream without decoding individual instructions.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_bytecode::pool::ConstantPool;
/// use aglais_xqvm_bytecode::program::Program;
///
/// let program = Program::new(ConstantPool::new(), vec![0x0Fu8]); // HALT
/// let bytes = program.encode();
/// let decoded = Program::decode(&bytes).unwrap();
/// assert_eq!(decoded.code(), &[0x0F]);
/// assert!(decoded.pool().is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pool: ConstantPool,
    code: Vec<u8>,
}

impl Program {
    /// Create a new program from a constant pool and raw instruction bytes.
    pub fn new(pool: ConstantPool, code: Vec<u8>) -> Self {
        Self { pool, code }
    }

    /// The constant pool associated with this program.
    pub fn pool(&self) -> &ConstantPool {
        &self.pool
    }

    /// The raw instruction bytes, without the pool header.
    pub fn code(&self) -> &[u8] {
        &self.code
    }

    /// Encode the program to a flat byte buffer.
    ///
    /// The output starts with a two-byte big-endian pool count, followed by
    /// each constant as an 8-byte big-endian `i64`, followed by the raw
    /// instruction bytes.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_bytecode::pool::ConstantPool;
    /// use aglais_xqvm_bytecode::program::Program;
    ///
    /// // Empty pool: first two bytes are 0x00 0x00.
    /// let prog = Program::new(ConstantPool::new(), vec![0x0F]);
    /// let bytes = prog.encode();
    /// assert_eq!(&bytes[..2], &[0x00, 0x00]);
    /// assert_eq!(bytes[2], 0x0F);
    /// ```
    pub fn encode(&self) -> Vec<u8> {
        let entries = self.pool.entries();
        let mut out = Vec::with_capacity(2 + entries.len() * 8 + self.code.len());
        let count = entries.len() as u16;
        out.extend_from_slice(&count.to_be_bytes());
        for &val in entries {
            out.extend_from_slice(&val.to_be_bytes());
        }
        out.extend_from_slice(&self.code);
        out
    }

    /// Decode a program from its encoded byte representation.
    ///
    /// Reads the 2-byte pool count, then the pool entries, and treats the
    /// remainder as instruction bytes. Individual instructions are not
    /// decoded here.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::PoolTruncated`] if `bytes` is shorter than
    /// the pool header requires.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_bytecode::program::{DecodeError, Program};
    ///
    /// // A slice with only 1 byte cannot hold the 2-byte pool count.
    /// assert!(matches!(
    ///     Program::decode(&[0x00]),
    ///     Err(DecodeError::PoolTruncated { required: 2, available: 1 }),
    /// ));
    /// ```
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 2 {
            return Err(DecodeError::PoolTruncated {
                required: 2,
                available: bytes.len(),
            });
        }
        let count_bytes: [u8; 2] = bytes
            .get(..2)
            .and_then(|s| s.try_into().ok())
            .unwrap_or_else(|| unreachable!("bytes.len() >= 2 checked above"));
        let count = u16::from_be_bytes(count_bytes) as usize;
        let pool_end = 2 + count * 8;
        if bytes.len() < pool_end {
            return Err(DecodeError::PoolTruncated {
                required: pool_end,
                available: bytes.len(),
            });
        }
        let mut pool = ConstantPool::new();
        for i in 0..count {
            let off = 2 + i * 8;
            let arr: [u8; 8] = bytes
                .get(off..off + 8)
                .and_then(|s| s.try_into().ok())
                .unwrap_or_else(|| unreachable!("off + 8 <= pool_end <= bytes.len()"));
            let val = i64::from_be_bytes(arr);
            // SAFETY: `count` came from a u16, so at most 65535 iterations --
            // exactly the capacity of ConstantPool. intern() cannot overflow.
            let _ = pool
                .intern(val)
                .unwrap_or_else(|_| unreachable!("pool overflow during decode"));
        }
        let code = bytes
            .get(pool_end..)
            .unwrap_or_else(|| unreachable!("pool_end <= bytes.len() checked above"))
            .to_vec();
        Ok(Self::new(pool, code))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(unused_results)]
mod tests {
    use super::*;

    fn make_pool(vals: &[i64]) -> ConstantPool {
        let mut p = ConstantPool::new();
        for &v in vals {
            p.intern(v).unwrap();
        }
        p
    }

    #[test]
    fn encode_decode_empty_program() {
        let prog = Program::new(ConstantPool::new(), vec![]);
        let bytes = prog.encode();
        assert_eq!(bytes, [0x00, 0x00]);
        assert_eq!(Program::decode(&bytes).unwrap(), prog);
    }

    #[test]
    fn encode_decode_pool_and_code() {
        let prog = Program::new(make_pool(&[42, -1]), vec![0x0F]);
        let bytes = prog.encode();
        let decoded = Program::decode(&bytes).unwrap();
        assert_eq!(decoded.pool().get(0), Some(42));
        assert_eq!(decoded.pool().get(1), Some(-1));
        assert_eq!(decoded.code(), &[0x0F]);
    }

    #[test]
    fn encode_has_correct_length() {
        // 2 (pool count) + 3 * 8 (entries) + 2 (code) = 28
        let prog = Program::new(make_pool(&[1, 2, 3]), vec![0x00, 0x0F]);
        assert_eq!(prog.encode().len(), 28);
    }

    #[test]
    fn decode_truncated_pool_header() {
        assert!(matches!(
            Program::decode(&[0x00]),
            Err(DecodeError::PoolTruncated {
                required: 2,
                available: 1
            })
        ));
    }

    #[test]
    fn decode_empty_bytes_is_error() {
        assert!(matches!(
            Program::decode(&[]),
            Err(DecodeError::PoolTruncated {
                required: 2,
                available: 0
            })
        ));
    }

    #[test]
    fn decode_truncated_pool_entries() {
        // Pool count says 1 entry (8 bytes) but only 5 bytes follow the header.
        let bytes = [0x00u8, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(matches!(
            Program::decode(&bytes),
            Err(DecodeError::PoolTruncated { required: 10, .. })
        ));
    }

    #[test]
    fn code_only_with_empty_pool() {
        let prog = Program::new(ConstantPool::new(), vec![0x0F, 0x00]);
        let decoded = Program::decode(&prog.encode()).unwrap();
        assert_eq!(decoded.code(), &[0x0F, 0x00]);
        assert!(decoded.pool().is_empty());
    }

    #[test]
    fn pool_entries_roundtrip_in_order() {
        let prog = Program::new(make_pool(&[100, 200, 300]), vec![]);
        let decoded = Program::decode(&prog.encode()).unwrap();
        assert_eq!(decoded.pool().entries(), &[100, 200, 300]);
    }
}
