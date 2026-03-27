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

//! Core type definitions for the XQVM bytecode.
//!
//! Re-exports the public symbols from the internal definition modules:
//! [`Instruction`], [`Opcode`], [`DecodeError`], and [`Register`].
//!
//! The [`opcodes!`](crate::opcodes) x-macro is exported at the crate root
//! via `#[macro_export]` and does not require this module to be in scope.

// `opcodes!` must be in scope before the modules that consume it.
#[macro_use]
mod table;
mod instruction;
mod opcode;
mod operand;
mod register_effect;

pub use self::{
    instruction::Instruction,
    opcode::{DecodeError, Opcode},
    operand::Register,
    register_effect::RegisterEffect,
};
