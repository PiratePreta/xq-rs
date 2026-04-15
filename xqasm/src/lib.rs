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
//! use xqasm::assemble_source;
//!
//! let program = assemble_source("PUSH 42\nHALT").unwrap();
//! assert_eq!(program.code()[0], 0x11); // PUSH1 opcode
//! assert_eq!(*program.code().last().unwrap(), 0xFF); // HALT opcode
//! ```
//!
//! For finer control, use [`parse`] and [`assemble`] separately.

// Error types carry NamedSource<Arc<str>> for miette source snippets.
// The extra size is acceptable on error paths in an assembler.
#![allow(clippy::result_large_err)]
// Private modules have doc tests that are only visible to maintainers.
#![allow(rustdoc::private_doc_tests)]

mod assembler;
mod ast;
mod error;
mod parser;

// ---------------------------------------------------------------------------
// Public API re-exports
// ---------------------------------------------------------------------------

pub use assembler::assemble;
pub use ast::{AsmLine, Operand, ParsedInstr};
pub use error::{AssembleError, Error, ParseError};
pub use parser::parse;

// ---------------------------------------------------------------------------
// Convenience entry point
// ---------------------------------------------------------------------------

/// Parse and assemble `source` into a [`xqvm::Program`] in one step.
///
/// This is a convenience wrapper around [`parse`] followed by [`assemble`].
///
/// # Errors
///
/// Returns [`Error`] on any parse or assembly failure.  The error message
/// includes the source line and column where the problem was detected.
///
/// # Examples
///
/// ```rust
/// use xqasm::assemble_source;
///
/// // A simple program: push two values, add them, halt.
/// let program = assemble_source("PUSH 5\nPUSH 3\nADD\nHALT").unwrap();
/// assert!(!program.code().is_empty());
/// ```
pub fn assemble_source(source: &str) -> Result<xqvm::Program, Error> {
    let lines = parse(source, "<input>")?;
    let program = assemble(&lines, source, "<input>")?;
    Ok(program)
}
