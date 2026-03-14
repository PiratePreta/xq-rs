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

//! Runtime error types for the XQVM interpreter.
//!
//! [`Error`] describes every fault that can occur during bytecode execution.
//! For rich terminal diagnostics, convert it to a [`RuntimeDiagnostic`] via
//! [`Error::into_diagnostic`], which disassembles the bytecode and points a
//! miette caret at the failing instruction.
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_vm::error::Error;
//! use aglais_xqvm_bytecode::{codec, types::Instruction};
//!
//! let bytecode: Vec<u8> = [Instruction::Push { imm: 0 }, Instruction::Halt {}]
//!     .iter()
//!     .flat_map(codec::encode)
//!     .collect();
//!
//! let err = Error::DivisionByZero { pos: 0 };
//! let diag = err.into_diagnostic(&bytecode, "prog.xqbc");
//! // diag implements miette::Diagnostic and can be returned from main()
//! ```

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

use aglais_xqvm_disasm::display::Disassembly;

/// Errors that can occur during XQVM bytecode execution.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    /// The value stack was empty when a pop was attempted.
    #[error("stack underflow at byte {pos:#06x}")]
    StackUnderflow { pos: usize },

    /// A register held the wrong value kind for the operation.
    #[error("register r{reg} holds {got}, expected {expected}")]
    RegisterType {
        reg: u8,
        expected: &'static str,
        got: &'static str,
    },

    /// Division or modulo by zero.
    #[error("division by zero at byte {pos:#06x}")]
    DivisionByZero { pos: usize },

    /// Vec index out of bounds.
    #[error("vec index {index} out of bounds (len {len}) at byte {pos:#06x}")]
    IndexOutOfBounds { pos: usize, index: i64, len: usize },

    /// NEXT or LVAL executed outside a loop.
    #[error("loop instruction at byte {pos:#06x} with no active loop")]
    NoActiveLoop { pos: usize },

    /// Jump target is outside the bytecode buffer.
    #[error("jump from {pos:#06x} to {target:#010x} is out of bounds")]
    BadJumpTarget { pos: usize, target: usize },

    /// The bytecode stream returned a decode error.
    #[error("bad opcode {byte:#04x} at byte {pos:#06x}")]
    BadOpcode { pos: usize, byte: u8 },

    /// The bytecode stream hit a truncated instruction.
    #[error("truncated instruction at byte {pos:#06x}")]
    TruncatedInstruction { pos: usize },

    /// Calldata index out of range.
    #[error("calldata index {index} out of range (len {len})")]
    CallDataIndex { index: i64, len: usize },

    /// Output index out of range.
    #[error("output index {index} out of range (len {len})")]
    OutputIndex { index: i64, len: usize },

    /// The model and sample sizes do not match.
    #[error("model has {model_size} variables but sample has {sample_len}")]
    SizeMismatch {
        model_size: usize,
        sample_len: usize,
    },

    /// Execution exceeded the configured step limit.
    #[error("step limit of {limit} exceeded")]
    StepLimitExceeded { limit: u64 },

    /// Left shift amount is negative or too large.
    #[error("invalid shift amount {amount} at byte {pos:#06x}")]
    InvalidShift { pos: usize, amount: i64 },

    /// The RESIZE instruction received non-positive dimensions.
    #[error("invalid grid dimensions {rows}x{cols} at byte {pos:#06x}")]
    InvalidGridDimensions { pos: usize, rows: i64, cols: i64 },
}

impl Error {
    /// Convert this error into a [`RuntimeDiagnostic`] with a disassembly
    /// listing as source context.
    ///
    /// `bytecode` is the program that was being executed when the fault
    /// occurred. `name` is used as the file name in the diagnostic output
    /// (e.g. `"prog.xqbc"` or `"<stdin>"`).
    ///
    /// When the error carries a byte position, the corresponding line of the
    /// disassembly is highlighted with a caret. Errors without a position
    /// (e.g. [`Error::StepLimitExceeded`]) produce a diagnostic with no
    /// source label.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_vm::vm::Vm;
    /// use aglais_xqvm_bytecode::builder::InstructionBuilder;
    ///
    /// fn run() -> miette::Result<()> {
    ///     let mut b = InstructionBuilder::new();
    ///     b.push(10).push(0).div().halt();
    ///     let bytecode = b.build().unwrap();
    ///
    ///     let mut vm = Vm::new();
    ///     vm.run(&bytecode)
    ///         .map_err(|e| e.into_diagnostic(&bytecode, "<inline>"))?;
    ///     Ok(())
    /// }
    /// ```
    pub fn into_diagnostic(self, bytecode: &[u8], name: &str) -> RuntimeDiagnostic {
        let disasm_text = Disassembly::new(bytecode).to_string();
        let span = self
            .byte_pos()
            .and_then(|pos| find_line_span(&disasm_text, pos));
        RuntimeDiagnostic {
            inner: self,
            disasm: NamedSource::new(name, disasm_text),
            span,
        }
    }

    /// Returns the byte offset embedded in this error, if any.
    fn byte_pos(&self) -> Option<usize> {
        match self {
            Self::StackUnderflow { pos }
            | Self::DivisionByZero { pos }
            | Self::NoActiveLoop { pos }
            | Self::BadOpcode { pos, .. }
            | Self::TruncatedInstruction { pos }
            | Self::InvalidShift { pos, .. }
            | Self::InvalidGridDimensions { pos, .. }
            | Self::BadJumpTarget { pos, .. }
            | Self::IndexOutOfBounds { pos, .. } => Some(*pos),
            Self::RegisterType { .. }
            | Self::CallDataIndex { .. }
            | Self::OutputIndex { .. }
            | Self::SizeMismatch { .. }
            | Self::StepLimitExceeded { .. } => None,
        }
    }
}

impl From<aglais_xqvm_bytecode::stream::Error> for Error {
    fn from(e: aglais_xqvm_bytecode::stream::Error) -> Self {
        use aglais_xqvm_bytecode::stream::Error as SE;
        match e {
            SE::UnknownOpcode { offset, byte } => Self::BadOpcode { pos: offset, byte },
            SE::TruncatedInstruction { offset } => Self::TruncatedInstruction { pos: offset },
            SE::SeekOutOfBounds { target, .. } => Self::BadJumpTarget { pos: 0, target },
        }
    }
}

// ---------------------------------------------------------------------------
// RuntimeDiagnostic
// ---------------------------------------------------------------------------

/// A runtime [`Error`] enriched with a disassembly listing as source context.
///
/// Construct it via [`Error::into_diagnostic`]. When the error carries a byte
/// offset, the corresponding disassembly line is highlighted with a caret.
///
/// `RuntimeDiagnostic` implements [`miette::Diagnostic`] so it can be
/// returned directly from a `fn main() -> miette::Result<()>`.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_vm::vm::Vm;
/// use aglais_xqvm_bytecode::builder::InstructionBuilder;
///
/// fn run() -> miette::Result<()> {
///     let mut b = InstructionBuilder::new();
///     b.push(3).push(4).add().halt();
///     let bytecode = b.build().unwrap();
///
///     let mut vm = Vm::new();
///     vm.run(&bytecode)
///         .map_err(|e| e.into_diagnostic(&bytecode, "<inline>"))?;
///     Ok(())
/// }
/// ```
#[derive(Debug, Error, Diagnostic)]
#[error("{inner}")]
#[diagnostic(code(xqvm::runtime_error))]
pub struct RuntimeDiagnostic {
    inner: Error,
    #[source_code]
    disasm: NamedSource<String>,
    #[label("execution failed here")]
    span: Option<SourceSpan>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find the byte range of the disassembly line that starts at `byte_pos`.
///
/// Each disassembly line has the form `  0x{offset:04X}:  ...`, so we search
/// for `"0x{byte_pos:04X}:"` and extend the span to cover the whole line.
/// Returns `None` when no matching line is found.
fn find_line_span(text: &str, byte_pos: usize) -> Option<SourceSpan> {
    let needle = format!("0x{byte_pos:04X}:");
    let match_start = text.find(&needle)?;

    let line_start = text[..match_start].rfind('\n').map(|n| n + 1).unwrap_or(0);

    let line_end = text[match_start..]
        .find('\n')
        .map(|n| match_start + n)
        .unwrap_or(text.len());

    Some(SourceSpan::from(line_start..line_end))
}
