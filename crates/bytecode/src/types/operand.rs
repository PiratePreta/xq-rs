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

/// An 8-bit register slot identifier (`r0`--`r255`).
///
/// A register can hold a value of runtime type `int`, `vec`, or `xqmx`.
/// Only `int` registers exchange values with the stack via `LOAD`/`STOW`.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_bytecode::types::Register;
///
/// let r = Register(0);
/// assert_eq!(r.slot(), 0);
///
/// let max = Register(255);
/// assert_eq!(max.slot(), 255);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Register(pub u8);

impl Register {
    /// Return the underlying 8-bit slot index.
    pub fn slot(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_slot_accessor() {
        assert_eq!(Register(0).slot(), 0);
        assert_eq!(Register(255).slot(), 255);
    }

    #[test]
    fn register_default_is_slot_zero() {
        assert_eq!(Register::default(), Register(0));
    }
}
