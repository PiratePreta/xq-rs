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

//! Constant pool for XQVM bytecode.
//!
//! A [`ConstantPool`] stores `i64` immediate values that are too large or too
//! frequent to embed inline in every [`PUSH`](crate::types::Instruction::Push)
//! instruction. Instead, a
//! [`PUSHC`](crate::types::Instruction::PushC) instruction references a value
//! by its pool index (`u16`), keeping the instruction stream compact when the
//! same constant appears repeatedly.
//!
//! Values are stored in insertion order and deduplicated: calling
//! [`intern`](ConstantPool::intern) twice with the same value returns the same
//! index both times without growing the pool.
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_bytecode::ConstantPool;
//!
//! let mut pool = ConstantPool::new();
//! let a = pool.intern(42).unwrap();
//! let b = pool.intern(42).unwrap(); // deduplicated
//! let c = pool.intern(-1).unwrap();
//!
//! assert_eq!(a, b);
//! assert_ne!(a, c);
//! assert_eq!(pool.get(a), Some(42));
//! assert_eq!(pool.get(c), Some(-1));
//! assert_eq!(pool.len(), 2);
//! ```

use std::collections::HashMap;

use thiserror::Error;

// ---------------------------------------------------------------------------
// PoolOverflow
// ---------------------------------------------------------------------------

/// Error returned when [`ConstantPool::intern`] is called on a full pool.
///
/// A pool holds at most 65535 distinct entries (the maximum `u16` value).
/// Attempting to add a 65536th distinct constant fails with this error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("constant pool is full (maximum 65535 distinct entries)")]
pub struct PoolOverflow;

// ---------------------------------------------------------------------------
// ConstantPool
// ---------------------------------------------------------------------------

/// A table of interned `i64` constants referenced by
/// [`PUSHC`](crate::types::Instruction::PushC) instructions.
///
/// Values are stored in insertion order. [`intern`](Self::intern) deduplicates:
/// calling it with the same value twice returns the same index both times.
/// Indices are `u16`, so the pool holds at most 65535 distinct values.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_bytecode::ConstantPool;
///
/// let mut pool = ConstantPool::new();
/// assert!(pool.is_empty());
///
/// let idx = pool.intern(1_000_000_000_000i64).unwrap();
/// assert_eq!(pool.get(idx), Some(1_000_000_000_000i64));
/// assert_eq!(pool.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConstantPool {
    entries: Vec<i64>,
    /// Reverse map for O(1) deduplication in `intern`.
    index: HashMap<i64, u16>,
}

impl ConstantPool {
    /// Create an empty constant pool.
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern `value` and return its pool index.
    ///
    /// If `value` is already in the pool, the existing index is returned
    /// without growing the pool. Otherwise a new entry is appended and
    /// its index returned.
    ///
    /// # Errors
    ///
    /// Returns [`PoolOverflow`] when the pool already contains 65535 distinct
    /// values and `value` is not among them.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_bytecode::ConstantPool;
    ///
    /// let mut pool = ConstantPool::new();
    /// let i0 = pool.intern(10).unwrap();
    /// let i1 = pool.intern(20).unwrap();
    /// let i0b = pool.intern(10).unwrap(); // deduplicates
    /// assert_eq!(i0, i0b);
    /// assert_ne!(i0, i1);
    /// ```
    pub fn intern(&mut self, value: i64) -> Result<u16, PoolOverflow> {
        if let Some(&idx) = self.index.get(&value) {
            return Ok(idx);
        }
        if self.entries.len() >= usize::from(u16::MAX) {
            return Err(PoolOverflow);
        }
        // SAFETY: len < u16::MAX (65535) so the cast is lossless.
        let idx = self.entries.len() as u16;
        self.entries.push(value);
        let _ = self.index.insert(value, idx);
        Ok(idx)
    }

    /// Return the value stored at `idx`, or `None` if `idx` is out of bounds.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_bytecode::ConstantPool;
    ///
    /// let mut pool = ConstantPool::new();
    /// let idx = pool.intern(99).unwrap();
    /// assert_eq!(pool.get(idx), Some(99));
    /// assert_eq!(pool.get(idx + 1), None);
    /// ```
    pub fn get(&self, idx: u16) -> Option<i64> {
        self.entries.get(usize::from(idx)).copied()
    }

    /// Number of distinct constants in the pool.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the pool contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The constants in insertion order.
    ///
    /// The position of each value in this slice is its pool index.
    pub fn entries(&self) -> &[i64] {
        &self.entries
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(unused_results)]
mod tests {
    use super::*;

    #[test]
    fn intern_deduplicates_values() {
        let mut pool = ConstantPool::new();
        let a = pool.intern(42).unwrap();
        let b = pool.intern(42).unwrap();
        assert_eq!(a, b);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn intern_returns_sequential_indices() {
        let mut pool = ConstantPool::new();
        let a = pool.intern(1).unwrap();
        let b = pool.intern(2).unwrap();
        let c = pool.intern(3).unwrap();
        assert_eq!(a, 0);
        assert_eq!(b, 1);
        assert_eq!(c, 2);
    }

    #[test]
    fn get_valid_index() {
        let mut pool = ConstantPool::new();
        let idx = pool.intern(99).unwrap();
        assert_eq!(pool.get(idx), Some(99));
    }

    #[test]
    fn get_out_of_range_returns_none() {
        let pool = ConstantPool::new();
        assert_eq!(pool.get(0), None);
    }

    #[test]
    fn len_and_is_empty() {
        let mut pool = ConstantPool::new();
        assert!(pool.is_empty());
        pool.intern(1).unwrap();
        assert_eq!(pool.len(), 1);
        assert!(!pool.is_empty());
    }

    #[test]
    fn entries_order_matches_insertion() {
        let mut pool = ConstantPool::new();
        pool.intern(10).unwrap();
        pool.intern(20).unwrap();
        pool.intern(30).unwrap();
        assert_eq!(pool.entries(), &[10, 20, 30]);
    }

    #[test]
    fn intern_at_capacity_returns_overflow() {
        let mut pool = ConstantPool::new();
        // Fill to exactly u16::MAX entries (65535).
        for i in 0..i64::from(u16::MAX) {
            pool.intern(i).unwrap();
        }
        assert_eq!(pool.len(), usize::from(u16::MAX));
        // One more distinct value should overflow.
        assert_eq!(pool.intern(i64::from(u16::MAX) + 1), Err(PoolOverflow));
    }

    #[test]
    fn intern_existing_value_at_capacity_succeeds() {
        let mut pool = ConstantPool::new();
        for i in 0..i64::from(u16::MAX) {
            pool.intern(i).unwrap();
        }
        // Interning an already-present value must still succeed.
        assert_eq!(pool.intern(0), Ok(0));
    }
}
