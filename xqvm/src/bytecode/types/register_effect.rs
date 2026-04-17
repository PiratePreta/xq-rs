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

//! Register effect metadata for instructions.

/// A compact set of register indices (0-2 elements, stack-allocated).
///
/// Used by [`Instruction::read_registers`](super::Instruction::read_registers)
/// and [`Instruction::written_registers`](super::Instruction::written_registers)
/// to report which registers an instruction accesses without heap allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterEffect {
    buf: [u8; 2],
    len: u8,
}

impl RegisterEffect {
    /// Empty set (no registers).
    pub const EMPTY: Self = Self {
        buf: [0; 2],
        len: 0,
    };

    /// Single-register set.
    pub const fn one(r: u8) -> Self {
        Self {
            buf: [r, 0],
            len: 1,
        }
    }

    /// Two-register set.
    pub const fn two(a: u8, b: u8) -> Self {
        Self {
            buf: [a, b],
            len: 2,
        }
    }

    /// View as a slice.
    pub fn as_slice(&self) -> &[u8] {
        // SAFETY: `len` is only set to 0, 1, or 2 by private constructors.
        #[expect(
            clippy::indexing_slicing,
            reason = "len is only ever set to 0, 1, or 2 by private constructors; [..len] is always in bounds"
        )]
        &self.buf[..self.len as usize]
    }

    /// Number of registers in the set.
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether the set is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_register_effect() {
        let e = RegisterEffect::EMPTY;
        assert!(e.is_empty());
        assert_eq!(e.len(), 0);
        assert_eq!(e.as_slice(), &[] as &[u8]);
    }

    #[test]
    fn one_register_effect() {
        let e = RegisterEffect::one(5);
        assert!(!e.is_empty());
        assert_eq!(e.len(), 1);
        assert_eq!(e.as_slice(), &[5]);
    }

    #[test]
    fn two_register_effect() {
        let e = RegisterEffect::two(3, 7);
        assert_eq!(e.len(), 2);
        assert_eq!(e.as_slice(), &[3, 7]);
    }
}
