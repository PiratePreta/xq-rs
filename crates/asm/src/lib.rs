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

//! Assembler for the Aglais X-Quadratic Virtual Machine (XQVM).
//!
//! This crate converts XQVM assembly source text into the binary bytecode
//! format consumed by the VM.
//!
//! # Language overview
//!
//! Each source line may contain:
//! - A **comment** starting with `;` (rest of line is ignored).
//! - A **label definition**: `name:` -- anchors the name to the current
//!   byte offset.
//! - An **instruction**: `MNEMONIC [operands...]`
//! - Any combination of the above on one line: `loop: PUSH -1 ; decrement`
//!
//! **Operand kinds:**
//!
//! | Syntax | Meaning |
//! |--------|---------|
//! | `r0` .. `r255` | Register slot operand |
//! | `42`, `-99`, `0xFF` | Signed integer literal |
//! | `label_name` | Label reference (only valid in `JUMP`/`JUMPI`) |
//!
//! # Entry point
//!
//! The primary API is [`assemble_source`]:
//!
//! ```rust
//! use aglais_xqvm_asm::assemble_source;
//!
//! let bytecode = assemble_source("PUSH 42\nHALT").unwrap();
//! assert_eq!(bytecode[0], 0x10); // PUSH opcode
//! assert_eq!(*bytecode.last().unwrap(), 0x0F); // HALT opcode
//! ```
//!
//! For finer control, use [`parser::parse`] and [`assembler::assemble`]
//! separately.

// Error types carry NamedSource<Arc<str>> for miette source snippets.
// The extra size is acceptable on error paths in an assembler.
#![allow(clippy::result_large_err)]

pub mod assembler;
pub mod ast;
pub mod error;
pub mod parser;

// ---------------------------------------------------------------------------
// Convenience entry point
// ---------------------------------------------------------------------------

/// Parse and assemble `source` into a binary bytecode buffer in one step.
///
/// This is a convenience wrapper around [`parser::parse`] followed by
/// [`assembler::assemble`].
///
/// # Errors
///
/// Returns [`error::Error`] on any parse or assembly failure.  The error
/// message includes the source line and column where the problem was detected.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_asm::assemble_source;
///
/// // A simple program: push two values, add them, halt.
/// let bytecode = assemble_source("PUSH 5\nPUSH 3\nADD\nHALT").unwrap();
/// assert!(!bytecode.is_empty());
/// ```
pub fn assemble_source(source: &str) -> Result<Vec<u8>, error::Error> {
    let lines = parser::parse(source, "<input>")?;
    let bytes = assembler::assemble(&lines, source, "<input>")?;
    Ok(bytes)
}
