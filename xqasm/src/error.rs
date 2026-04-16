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
//!
//! All error types implement [`miette::Diagnostic`], so callers that use
//! `miette::Result` receive a source-code snippet with a caret pointing to
//! the exact token that caused the failure.

use std::sync::Arc;

use miette::{Diagnostic, NamedSource, SourceSpan};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Source helper
// ---------------------------------------------------------------------------

/// Bundles source text and its display name for diagnostic construction.
#[derive(Clone, Copy)]
pub(crate) struct Source<'a> {
    pub text: &'a str,
    pub name: &'a str,
}

// ---------------------------------------------------------------------------
// Internal span helpers
// ---------------------------------------------------------------------------

/// Build a [`NamedSource`] from a [`Source`].
pub(crate) fn make_src(src: Source<'_>) -> NamedSource<Arc<str>> {
    NamedSource::new(src.name, Arc::from(src.text))
}

/// Build a [`SourceSpan`] from a byte `offset` and byte `len`.
pub(crate) fn make_span(offset: usize, len: usize) -> SourceSpan {
    (offset, len).into()
}

// ---------------------------------------------------------------------------
// ParseError
// ---------------------------------------------------------------------------

/// A syntax error produced by the parser.
///
/// The embedded [`NamedSource`] and [`SourceSpan`] let miette render a
/// source snippet with a caret when the error is displayed.
///
/// # Examples
///
/// ```rust
/// use xqasm::parse;
///
/// let err = parse("@@@", "<test>").unwrap_err();
/// assert!(err.to_string().contains("error"));
/// ```
#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
#[diagnostic(code(xqasm::parse_error))]
pub struct ParseError {
    /// Human-readable description of the error.
    pub message: String,
    /// Source text used for snippet rendering.
    #[source_code]
    pub(crate) src: NamedSource<Arc<str>>,
    /// Span of the offending token.
    #[label("syntax error")]
    pub(crate) span: SourceSpan,
}

// ---------------------------------------------------------------------------
// AssembleError
// ---------------------------------------------------------------------------

/// A semantic error produced by the assembler.
///
/// Each variant carries a [`NamedSource`] and a [`SourceSpan`] so that miette
/// can display the failing token inline in the source listing.
#[derive(Debug, Error, Diagnostic)]
pub enum AssembleError {
    /// The mnemonic string does not correspond to any XQVM opcode.
    #[error("unknown mnemonic `{mnemonic}`")]
    #[diagnostic(code(xqasm::unknown_mnemonic))]
    UnknownMnemonic {
        /// The unrecognised mnemonic string.
        mnemonic: String,
        /// Source text for diagnostic rendering.
        #[source_code]
        src: NamedSource<Arc<str>>,
        /// Span of the failing token.
        #[label("unknown mnemonic")]
        span: SourceSpan,
    },

    /// The instruction was given the wrong number of operands.
    #[error("'{mnemonic}' expects {expected} operand(s), got {got}")]
    #[diagnostic(code(xqasm::wrong_operand_count))]
    WrongOperandCount {
        /// Mnemonic of the failing instruction.
        mnemonic: String,
        /// Number of operands the instruction requires.
        expected: usize,
        /// Number of operands actually supplied.
        got: usize,
        /// Source text for diagnostic rendering.
        #[source_code]
        src: NamedSource<Arc<str>>,
        /// Span of the failing token.
        #[label("wrong number of operands")]
        span: SourceSpan,
    },

    /// An operand was of the wrong kind (e.g. an integer where a register
    /// was expected).
    #[error("operand '{field}' of '{mnemonic}' must be a {expected_kind}")]
    #[diagnostic(code(xqasm::wrong_operand_kind))]
    WrongOperandKind {
        /// Mnemonic of the failing instruction.
        mnemonic: String,
        /// Name of the field that has the wrong kind.
        field: String,
        /// Description of the expected kind.
        expected_kind: String,
        /// Source text for diagnostic rendering.
        #[source_code]
        src: NamedSource<Arc<str>>,
        /// Span of the failing token.
        #[label("wrong operand kind")]
        span: SourceSpan,
    },

    /// A register literal was out of the valid `[0, 255]` range.
    #[error("register index {value} is out of range [0, 255]")]
    #[diagnostic(code(xqasm::register_out_of_range))]
    RegisterOutOfRange {
        /// The out-of-range value.
        value: u64,
        /// Source text for diagnostic rendering.
        #[source_code]
        src: NamedSource<Arc<str>>,
        /// Span of the failing token.
        #[label("out of range")]
        span: SourceSpan,
    },

    /// An integer literal could not be converted to the required type.
    #[error(
        "integer {value} does not fit in {target_type} \
         (field '{field}' of '{mnemonic}')"
    )]
    #[diagnostic(code(xqasm::integer_out_of_range))]
    IntegerOutOfRange {
        /// The problematic integer value.
        value: i64,
        /// Rust type name of the target operand type.
        target_type: &'static str,
        /// Field name.
        field: String,
        /// Mnemonic.
        mnemonic: String,
        /// Source text for diagnostic rendering.
        #[source_code]
        src: NamedSource<Arc<str>>,
        /// Span of the failing token.
        #[label("out of range")]
        span: SourceSpan,
    },

    /// A label reference was used in a `JUMP`/`JUMPI` but never defined.
    #[error("undefined label '.{label}'")]
    #[diagnostic(code(xqasm::undefined_label))]
    UndefinedLabel {
        /// The numeric label index that was referenced but never defined.
        label: u16,
        /// Source text for diagnostic rendering.
        #[source_code]
        src: NamedSource<Arc<str>>,
        /// Span of the failing token.
        #[label("label not defined")]
        span: SourceSpan,
    },

    /// A label was defined more than once in the same source.
    #[error("label '.{label}' is defined more than once")]
    #[diagnostic(code(xqasm::duplicate_label))]
    DuplicateLabel {
        /// The duplicated label index.
        label: u16,
        /// Source text for diagnostic rendering.
        #[source_code]
        src: NamedSource<Arc<str>>,
        /// Span of the failing token.
        #[label("duplicate definition")]
        span: SourceSpan,
    },

    /// A label was placed but never referenced by any `JUMP`/`JUMPI`.
    #[error("label '.{label}' is defined but never used")]
    #[diagnostic(code(xqasm::unused_label))]
    UnusedLabel {
        /// The unused label index.
        label: u16,
        /// Source text for diagnostic rendering.
        #[source_code]
        src: NamedSource<Arc<str>>,
        /// Span of the label definition.
        #[label("unused label")]
        span: SourceSpan,
    },

    /// The program contains more than `u16::MAX + 1` labels (`TARGET`s),
    /// which exceeds the wire-format limit on sequential target ids.
    #[error("too many TARGETs: {count} (max {})", u16::MAX as usize + 1)]
    #[diagnostic(code(xqasm::too_many_targets))]
    TooManyTargets {
        /// Total number of placed labels in the program.
        count: usize,
        /// Source text for diagnostic rendering.
        #[source_code]
        src: NamedSource<Arc<str>>,
    },
}

// ---------------------------------------------------------------------------
// Top-level Error
// ---------------------------------------------------------------------------

/// Top-level error type for the `assemble_source` function.
///
/// Implements [`miette::Diagnostic`] by forwarding to the inner parse or
/// assemble error, so callers see the same source snippet regardless of which
/// phase failed.
#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    /// A syntax error from the pest parser.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),
    /// A semantic error from the assembler.
    #[error(transparent)]
    #[diagnostic(transparent)]
    Assemble(#[from] AssembleError),
}
