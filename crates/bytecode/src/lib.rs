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
//! This crate is a pure definition crate with no execution logic. It exposes
//! one public module and one re-exported macro:
//!
//! | Item | Description |
//! |---|---|
//! | [`opcodes!`] | X-macro -- the single source of truth for the opcode table |
//! | [`Opcode`] | `#[repr(u8)]` enum |
//! | [`Instruction`] | Fully decoded instruction with operands |
//! | [`Register`] | 8-bit register slot operand |
//! | [`Program`] | Complete program: raw instruction bytes |
//! | [`InstructionBuilder`] | Fluent bytecode assembler |
//! | [`InstructionStream`] | Incremental seekable reader |
//! | [`codec`] | [`codec::encode`] / [`codec::decode`] -- binary wire format |
//! | [`error`] | All public error types |
//!
//! # The x-macro pattern
//!
//! All data types are derived from the [`opcodes!`] callback macro. Pass the
//! name of any `macro_rules!` you define as the argument; it will be invoked
//! with the full 76-entry opcode table:
//!
//! ```rust
//! macro_rules! list_mnemonics {
//!     ( $( ($code:literal, $var:ident, $mnem:literal, $doc:literal, {$($f:tt)*}) ),* $(,)? ) => {
//!         &[ $( $mnem ),* ] as &[&str]
//!     }
//! }
//!
//! let mnemonics = aglais_xqvm_bytecode::opcodes!(list_mnemonics);
//! assert_eq!(mnemonics.len(), 87);
//! assert!(mnemonics.contains(&"ENERGY"));
//! assert!(mnemonics.contains(&"PUSH1"));
//! ```
//!
//! # Quick start
//!
//! ```rust
//! use aglais_xqvm_bytecode::{Instruction, Opcode, Register};
//!
//! let program: &[Instruction] = &[
//!     Instruction::Push1  { val: [0] },
//!     Instruction::Push1  { val: [10] },
//!     Instruction::Range  {},
//!     Instruction::LVal   { reg: Register(0) },
//!     Instruction::Load   { reg: Register(0) },
//!     Instruction::Push1  { val: [5] },
//!     Instruction::Gt     {},
//!     Instruction::JumpI1 { label: 0u8 },
//!     Instruction::Next   {},
//!     Instruction::Target {},
//!     Instruction::Halt   {},
//! ];
//!
//! assert_eq!(program[0].opcode(), Opcode::Push1);
//! assert_eq!(program[0].opcode() as u8, 0x11);
//! assert_eq!(program[0].mnemonic(), "PUSH1");
//! ```

// No standard library when the `std` feature is disabled (e.g. WASM targets).
// The `alloc` crate provides heap types (`Vec`, `String`, `BTreeMap`, ...).
#![cfg_attr(not(feature = "std"), no_std)]
// Private modules have doc tests that are only visible to maintainers.
#![allow(rustdoc::private_doc_tests)]

#[cfg(not(feature = "std"))]
extern crate alloc;

// `opcodes!` is defined inside `types::table` with `#[macro_export]`, so it
// lands at the crate root automatically.
#[macro_use]
mod types;
mod builder;
pub mod codec;
pub mod error;
mod jump_table;
mod program;
mod stream;

// ---------------------------------------------------------------------------
// Public API re-exports
// ---------------------------------------------------------------------------

pub use builder::{InstructionBuilder, LabelId};
pub use jump_table::JumpTable;
pub use program::Program;
pub use stream::InstructionStream;
pub use types::{Instruction, Opcode, Register, RegisterEffect};
