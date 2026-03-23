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

// Private modules have doc tests that are only visible to maintainers.
#![allow(rustdoc::private_doc_tests)]

//! Disassembler for the Aglais X-Quadratic Virtual Machine (XQVM).
//!
//! This crate converts raw XQVM bytecode buffers into human-readable listings.
//! The only public API is [`Disassembly`].
//!
//! # Quick start
//!
//! ```rust
//! use aglais_xqvm_bytecode::{Instruction, codec};
//! use aglais_xqvm_disasm::Disassembly;
//!
//! let program = [
//!     Instruction::PushC1 { val: [5] },
//!     Instruction::PushC1 { val: [0xFF] }, // -1 as i8
//!     Instruction::Add  {},
//!     Instruction::Halt {},
//! ];
//! let buf: Vec<u8> = program.iter().flat_map(|i| codec::encode(i)).collect();
//!
//! let listing = Disassembly::new(&buf).to_string();
//! assert!(listing.contains("PUSHC_1"));
//! assert!(listing.contains("HALT"));
//! ```

mod display;

pub use display::Disassembly;
