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

//! Jump table: maps label indices to basic-block byte ranges.
//!
//! A [`JumpTable`] is a sequence of [`JumpEntry`] triples
//! `(label, start, end)` where `start` and `end` are absolute byte offsets
//! into the instruction stream. The label index is a zero-based `u16` that
//! `JUMP` and `JUMPI` operands reference directly.
//!
//! ## Wire format
//!
//! The jump table is serialized immediately before the instruction stream:
//!
//! | Field | Type | Bytes |
//! |---|---|---|
//! | `entry_count` | u16 BE | 2 |
//! | Per entry: `label` | u16 BE | 2 |
//! | Per entry: `start` | u32 BE | 4 |
//! | Per entry: `end` | u32 BE | 4 |
//!
//! Total: `2 + 10 * entry_count` bytes.
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_bytecode::JumpTable;
//!
//! let table = JumpTable::default();
//! assert!(table.is_empty());
//! let bytes = table.encode();
//! let (decoded, consumed) = JumpTable::decode(&bytes).unwrap();
//! assert_eq!(consumed, 2);
//! assert!(decoded.is_empty());
//! ```

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use thiserror::Error;

// ---------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------

/// A single jump-table entry: maps a label index to a byte range.
///
/// `start` is the offset of the first byte of the basic block.
/// `end` is one past the last byte (exclusive), so the block spans
/// `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JumpEntry {
    /// Zero-based label index.
    pub label: u16,
    /// Start offset (inclusive) of the basic block in the instruction stream.
    pub start: u32,
    /// End offset (exclusive) of the basic block in the instruction stream.
    pub end: u32,
}

/// Byte size of a single wire-encoded entry (2 + 4 + 4).
const ENTRY_SIZE: usize = 10;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Error returned when decoding a [`JumpTable`] from bytes.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeError {
    /// The input is too short to contain the entry-count header.
    #[error("jump table truncated: need at least 2 bytes, got {len}")]
    TruncatedHeader {
        /// Actual byte length available.
        len: usize,
    },

    /// The input is too short for the declared number of entries.
    #[error(
        "jump table truncated: header declares {count} entries \
         ({expected} bytes) but only {available} bytes remain"
    )]
    TruncatedEntries {
        /// Declared entry count.
        count: u16,
        /// Bytes required for the entries.
        expected: usize,
        /// Bytes actually available after the header.
        available: usize,
    },
}

// ---------------------------------------------------------------------------
// JumpTable
// ---------------------------------------------------------------------------

/// A collection of basic-block descriptors used by `JUMP` / `JUMPI`.
///
/// Entries are indexed by their `label` field: `get(label)` returns the
/// entry whose `label` matches. Labels form a dense `0..N` range, so
/// lookup is O(1) via direct indexing.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_bytecode::{JumpTable, JumpEntry};
///
/// let entries = vec![
///     JumpEntry { label: 0, start: 0, end: 5 },
///     JumpEntry { label: 1, start: 5, end: 12 },
/// ];
/// let table = JumpTable::new(entries);
/// assert_eq!(table.len(), 2);
/// assert_eq!(table.get(0).unwrap().start, 0);
/// assert_eq!(table.get(1).unwrap().end, 12);
/// assert!(table.get(2).is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JumpTable {
    entries: Vec<JumpEntry>,
}

impl JumpTable {
    /// Create a jump table from a list of entries.
    pub fn new(entries: Vec<JumpEntry>) -> Self {
        Self { entries }
    }

    /// Look up a jump-table entry by label index.
    ///
    /// Returns `None` if `label` is out of range.
    pub fn get(&self, label: u16) -> Option<&JumpEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    /// The number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table is empty (no basic blocks).
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Borrow the entries slice.
    pub fn entries(&self) -> &[JumpEntry] {
        &self.entries
    }

    /// Encode the jump table to bytes (big-endian wire format).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_bytecode::{JumpTable, JumpEntry};
    ///
    /// let table = JumpTable::new(vec![
    ///     JumpEntry { label: 0, start: 0, end: 10 },
    /// ]);
    /// let bytes = table.encode();
    /// assert_eq!(bytes.len(), 2 + 10); // header + 1 entry
    /// ```
    pub fn encode(&self) -> Vec<u8> {
        let count = self.entries.len() as u16;
        let mut buf = Vec::with_capacity(2 + self.entries.len() * ENTRY_SIZE);
        buf.extend_from_slice(&count.to_be_bytes());
        for entry in &self.entries {
            buf.extend_from_slice(&entry.label.to_be_bytes());
            buf.extend_from_slice(&entry.start.to_be_bytes());
            buf.extend_from_slice(&entry.end.to_be_bytes());
        }
        buf
    }

    /// Decode a jump table from the start of `bytes`.
    ///
    /// Returns the decoded table and the number of bytes consumed.
    ///
    /// # Errors
    ///
    /// - [`DecodeError::TruncatedHeader`] -- fewer than 2 bytes.
    /// - [`DecodeError::TruncatedEntries`] -- not enough bytes for the
    ///   declared entry count.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), DecodeError> {
        if bytes.len() < 2 {
            return Err(DecodeError::TruncatedHeader { len: bytes.len() });
        }
        let count = u16::from_be_bytes([*bytes.first().unwrap_or(&0), *bytes.get(1).unwrap_or(&0)]);
        let body_len = usize::from(count) * ENTRY_SIZE;
        let available = bytes.len() - 2;
        if available < body_len {
            return Err(DecodeError::TruncatedEntries {
                count,
                expected: body_len,
                available,
            });
        }

        let mut entries = Vec::with_capacity(usize::from(count));
        let mut pos = 2;
        for _ in 0..count {
            let label = u16::from_be_bytes([
                *bytes.get(pos).unwrap_or(&0),
                *bytes.get(pos + 1).unwrap_or(&0),
            ]);
            let start = u32::from_be_bytes([
                *bytes.get(pos + 2).unwrap_or(&0),
                *bytes.get(pos + 3).unwrap_or(&0),
                *bytes.get(pos + 4).unwrap_or(&0),
                *bytes.get(pos + 5).unwrap_or(&0),
            ]);
            let end = u32::from_be_bytes([
                *bytes.get(pos + 6).unwrap_or(&0),
                *bytes.get(pos + 7).unwrap_or(&0),
                *bytes.get(pos + 8).unwrap_or(&0),
                *bytes.get(pos + 9).unwrap_or(&0),
            ]);
            entries.push(JumpEntry { label, start, end });
            pos += ENTRY_SIZE;
        }

        Ok((Self { entries }, pos))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn empty_table_roundtrip() {
        let table = JumpTable::default();
        let bytes = table.encode();
        assert_eq!(bytes, [0x00, 0x00]);
        let (decoded, consumed) = JumpTable::decode(&bytes).unwrap();
        assert_eq!(consumed, 2);
        assert_eq!(decoded, table);
    }

    #[test]
    fn single_entry_roundtrip() {
        let table = JumpTable::new(vec![JumpEntry {
            label: 0,
            start: 0,
            end: 42,
        }]);
        let bytes = table.encode();
        assert_eq!(bytes.len(), 12);
        let (decoded, consumed) = JumpTable::decode(&bytes).unwrap();
        assert_eq!(consumed, 12);
        assert_eq!(decoded, table);
    }

    #[test]
    fn multi_entry_roundtrip() {
        let table = JumpTable::new(vec![
            JumpEntry {
                label: 0,
                start: 0,
                end: 10,
            },
            JumpEntry {
                label: 1,
                start: 10,
                end: 25,
            },
            JumpEntry {
                label: 2,
                start: 25,
                end: 30,
            },
        ]);
        let bytes = table.encode();
        let (decoded, consumed) = JumpTable::decode(&bytes).unwrap();
        assert_eq!(consumed, 2 + 3 * 10);
        assert_eq!(decoded, table);
    }

    #[test]
    fn get_by_label() {
        let table = JumpTable::new(vec![
            JumpEntry {
                label: 0,
                start: 0,
                end: 5,
            },
            JumpEntry {
                label: 1,
                start: 5,
                end: 12,
            },
        ]);
        assert_eq!(table.get(0).unwrap().start, 0);
        assert_eq!(table.get(1).unwrap().end, 12);
        assert!(table.get(2).is_none());
    }

    #[test]
    fn truncated_header_error() {
        assert!(matches!(
            JumpTable::decode(&[0x00]),
            Err(DecodeError::TruncatedHeader { len: 1 })
        ));
    }

    #[test]
    fn truncated_entries_error() {
        // Header says 1 entry (10 bytes) but only 5 available.
        let bytes = [0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(matches!(
            JumpTable::decode(&bytes),
            Err(DecodeError::TruncatedEntries { count: 1, .. })
        ));
    }

    #[test]
    fn decode_with_trailing_bytes() {
        let table = JumpTable::new(vec![JumpEntry {
            label: 0,
            start: 0,
            end: 5,
        }]);
        let mut bytes = table.encode();
        bytes.extend_from_slice(&[0xFF, 0xFE]); // trailing code bytes
        let (decoded, consumed) = JumpTable::decode(&bytes).unwrap();
        assert_eq!(consumed, 12);
        assert_eq!(decoded, table);
    }
}
