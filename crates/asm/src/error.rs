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

//! Error types for the XQVM assembler.

use thiserror::Error;

// ---------------------------------------------------------------------------
// ParseError
// ---------------------------------------------------------------------------

/// A syntax error produced by the parser.
#[derive(Debug, Error)]
#[error("parse error at line {line}, col {col}: {message}")]
pub struct ParseError {
    /// 1-based line number where the error occurred.
    pub line: usize,
    /// 1-based column number where the error occurred.
    pub col: usize,
    /// Human-readable description of the error.
    pub message: String,
}

// ---------------------------------------------------------------------------
// AssembleError
// ---------------------------------------------------------------------------

/// A semantic error produced by the assembler.
#[derive(Debug, Error)]
pub enum AssembleError {
    /// The mnemonic string does not correspond to any XQVM opcode.
    #[error("line {line}: unknown mnemonic '{mnemonic}'")]
    UnknownMnemonic {
        /// The unrecognised mnemonic string.
        mnemonic: String,
        /// 1-based source line number.
        line: usize,
        /// 1-based source column.
        col: usize,
    },

    /// The instruction was given the wrong number of operands.
    #[error("line {line}: '{mnemonic}' expects {expected} operand(s), got {got}")]
    WrongOperandCount {
        /// Mnemonic of the failing instruction.
        mnemonic: String,
        /// Number of operands the instruction requires.
        expected: usize,
        /// Number of operands actually supplied.
        got: usize,
        /// 1-based source line number.
        line: usize,
        /// 1-based source column.
        col: usize,
    },

    /// An operand was of the wrong kind (e.g. an integer where a register
    /// was expected).
    #[error("line {line}: operand '{field}' of '{mnemonic}' must be a {expected_kind}")]
    WrongOperandKind {
        /// Mnemonic of the failing instruction.
        mnemonic: String,
        /// Name of the field that has the wrong kind.
        field: String,
        /// Description of the expected kind.
        expected_kind: String,
        /// 1-based source line number.
        line: usize,
        /// 1-based source column.
        col: usize,
    },

    /// A register literal was out of the valid `[0, 255]` range.
    #[error("line {line}: register index {value} is out of range [0, 255]")]
    RegisterOutOfRange {
        /// The out-of-range value.
        value: u64,
        /// 1-based source line.
        line: usize,
        /// 1-based source column.
        col: usize,
    },

    /// An integer literal could not be converted to the required type.
    #[error(
        "line {line}: integer {value} does not fit in {target_type} \
         (field '{field}' of '{mnemonic}')"
    )]
    IntegerOutOfRange {
        /// The problematic integer value.
        value: i64,
        /// Rust type name of the target operand type.
        target_type: &'static str,
        /// Field name.
        field: String,
        /// Mnemonic.
        mnemonic: String,
        /// 1-based source line.
        line: usize,
        /// 1-based source column.
        col: usize,
    },

    /// A label reference was used in a `JUMP`/`JUMPI` but never defined.
    #[error("line {line}: undefined label '{label}'")]
    UndefinedLabel {
        /// The label name that was referenced but never defined.
        label: String,
        /// 1-based source line.
        line: usize,
        /// 1-based source column.
        col: usize,
    },

    /// A label was defined more than once in the same source.
    #[error("label '{label}' is defined more than once (second definition at line {line})")]
    DuplicateLabel {
        /// The duplicated label name.
        label: String,
        /// 1-based source line of the second definition.
        line: usize,
        /// 1-based source column of the second definition.
        col: usize,
    },

    /// The byte offset between a jump site and its target exceeds the
    /// `i16` range `[-32768, 32767]`.
    #[error(
        "line {line}: jump offset {delta} to label '{label}' \
         does not fit in i16 (max range: -32768..=32767)"
    )]
    JumpOffsetOverflow {
        /// The label name.
        label: String,
        /// The computed offset that overflowed.
        delta: i64,
        /// 1-based source line of the jump instruction.
        line: usize,
        /// 1-based source column.
        col: usize,
    },
}

// ---------------------------------------------------------------------------
// Top-level Error
// ---------------------------------------------------------------------------

/// Top-level error type for the `assemble_source` function.
#[derive(Debug, Error)]
pub enum Error {
    /// A syntax error from the pest parser.
    #[error(transparent)]
    Parse(#[from] ParseError),
    /// A semantic error from the assembler.
    #[error(transparent)]
    Assemble(#[from] AssembleError),
}
