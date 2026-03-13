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

//! Bytecode definition for the Aglais X-Quadratic Virtual Machine (XQVM).
//!
//! This crate defines the opcode table, instruction types, and binary
//! serialization for the XQVM bytecode format.
//!
//! | Item | Description |
//! |---|---|
//! | [`opcodes!`] | X-macro -- the single source of truth for the opcode table |
//! | [`types::Opcode`] | `#[repr(u8)]` enum and [`types::DecodeError`] |
//! | [`types::Instruction`] | Fully decoded instruction with operands |
//! | [`types::Register`] | 8-bit register slot operand |
//! | [`codec`] | [`codec::encode`] / [`codec::decode`] -- binary wire format |
//!
//! # The x-macro pattern
//!
//! All data types are derived from the [`opcodes!`] callback macro. Pass the
//! name of any `macro_rules!` you define as the argument; it will be invoked
//! with the full 68-entry opcode table:
//!
//! ```rust
//! macro_rules! list_mnemonics {
//!     ( $( ($code:literal, $var:ident, $mnem:literal, $doc:literal, {$($f:tt)*}) ),* $(,)? ) => {
//!         &[ $( $mnem ),* ] as &[&str]
//!     }
//! }
//!
//! let mnemonics = aglais_xqvm_bytecode::opcodes!(list_mnemonics);
//! assert_eq!(mnemonics.len(), 68);
//! assert!(mnemonics.contains(&"ENERGY"));
//! ```
//!
//! # Quick start
//!
//! ```rust
//! use aglais_xqvm_bytecode::types::{Instruction, Opcode, Register};
//!
//! let program: &[Instruction] = &[
//!     Instruction::Push   { imm: 0 },
//!     Instruction::Push   { imm: 10 },
//!     Instruction::Range  {},
//!     Instruction::LVal   { reg: Register(0) },
//!     Instruction::Load   { reg: Register(0) },
//!     Instruction::Push   { imm: 5 },
//!     Instruction::Gt     {},
//!     Instruction::JumpI  { offset: 1i16 },
//!     Instruction::Next   {},
//!     Instruction::Target {},
//!     Instruction::Halt   {},
//! ];
//!
//! assert_eq!(program[0].opcode(), Opcode::Push);
//! assert_eq!(program[0].opcode() as u8, 0x10);
//! assert_eq!(program[0].mnemonic(), "PUSH");
//! ```

// `opcodes!` is defined inside `types::table` with `#[macro_export]`, so it
// lands at the crate root automatically.
#[macro_use]
pub mod types;
pub mod codec;
