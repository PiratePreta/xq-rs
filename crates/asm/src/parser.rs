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

//! Pest-based parser for XQVM assembly source.
//!
//! Converts raw text into a flat list of [`AsmLine`] values.  No semantic
//! validation is done here; the assembler handles unknown mnemonics, wrong
//! operand counts, and similar errors.
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_asm::parser::parse;
//! use aglais_xqvm_asm::ast::{AsmLine, Operand};
//!
//! let lines = parse("PUSH 42\nHALT", "<test>").unwrap();
//! assert_eq!(lines.len(), 2);
//! ```

use pest::Parser;

use crate::ast::{AsmLine, Operand, ParsedInstr};
use crate::error::{ParseError, make_span, make_src};

// ---------------------------------------------------------------------------
// Pest parser generated from grammar.pest
// ---------------------------------------------------------------------------

// The pest_derive macro generates a public `Rule` enum and associated items
// that cannot carry doc comments.  Wrapping the derive in a private module
// silences the missing_docs lint for the generated code without disabling it
// for the rest of the parser module.
mod generated {
    #![allow(missing_docs, unreachable_pub)]
    use pest_derive::Parser;

    #[derive(Parser)]
    #[grammar = "src/grammar.pest"]
    pub struct AsmParser;
}

use generated::AsmParser;
use generated::Rule;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse `source` into a flat list of [`AsmLine`] values.
///
/// `name` is used as the file name in diagnostic output (e.g. `"prog.xqasm"`
/// or `"<input>"`).  Blank lines and comment-only lines produce no entries.
/// A line that contains both a label definition and an instruction produces
/// two entries: first a [`AsmLine::LabelDef`], then an
/// [`AsmLine::Instruction`].
///
/// # Errors
///
/// Returns [`ParseError`] if the input does not conform to the grammar.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_asm::parser::parse;
/// use aglais_xqvm_asm::ast::{AsmLine, Operand};
///
/// let lines = parse("loop:\n  JUMPI loop", "<test>").unwrap();
/// assert!(matches!(lines[0], AsmLine::LabelDef { .. }));
/// assert!(matches!(lines[1], AsmLine::Instruction(_)));
/// ```
pub fn parse(source: &str, name: &str) -> Result<Vec<AsmLine>, ParseError> {
    let pairs = AsmParser::parse(Rule::program, source).map_err(|e| {
        let (line, col) = match e.line_col {
            pest::error::LineColLocation::Pos((l, c)) => (l, c),
            pest::error::LineColLocation::Span((l, c), _) => (l, c),
        };
        ParseError {
            message: e.variant.to_string(),
            src: make_src(source, name),
            span: make_span(source, line, col, 1),
        }
    })?;

    let mut out = Vec::new();

    for pair in pairs {
        if pair.as_rule() != Rule::program {
            continue;
        }
        for line_pair in pair.into_inner() {
            if line_pair.as_rule() != Rule::line {
                continue;
            }
            visit_line(line_pair, source, name, &mut out)?;
        }
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn visit_line(
    line_pair: pest::iterators::Pair<'_, Rule>,
    source: &str,
    name: &str,
    out: &mut Vec<AsmLine>,
) -> Result<(), ParseError> {
    for inner in line_pair.into_inner() {
        match inner.as_rule() {
            Rule::label_def => {
                let (line, col) = inner.line_col();
                let label_name = inner
                    .into_inner()
                    .next()
                    .unwrap_or_else(|| {
                        unreachable!("grammar guarantees label_def contains label_id")
                    })
                    .as_str()
                    .to_string();
                out.push(AsmLine::LabelDef {
                    name: label_name,
                    line,
                    col,
                });
            }
            Rule::instruction => {
                out.push(visit_instruction(inner, source, name)?);
            }
            _ => {}
        }
    }
    Ok(())
}

fn visit_instruction(
    pair: pest::iterators::Pair<'_, Rule>,
    source: &str,
    name: &str,
) -> Result<AsmLine, ParseError> {
    let (line, col) = pair.line_col();
    let mut inner = pair.into_inner();

    let mnemonic_pair = inner
        .next()
        .unwrap_or_else(|| unreachable!("grammar guarantees instruction starts with mnemonic"));
    let mnemonic = mnemonic_pair.as_str().to_string();

    let mut operands = Vec::new();
    for op_pair in inner {
        if op_pair.as_rule() == Rule::operand {
            operands.push(visit_operand(op_pair, source, name)?);
        }
    }

    Ok(AsmLine::Instruction(ParsedInstr {
        mnemonic,
        operands,
        line,
        col,
    }))
}

fn visit_operand(
    pair: pest::iterators::Pair<'_, Rule>,
    source: &str,
    name: &str,
) -> Result<Operand, ParseError> {
    let inner = pair
        .into_inner()
        .next()
        .unwrap_or_else(|| unreachable!("grammar guarantees operand has one inner rule"));
    let (line, col) = inner.line_col();
    let text = inner.as_str();

    match inner.as_rule() {
        Rule::register => {
            let digits = &text[1..]; // strip leading 'r'
            let slot: u64 = digits.parse().map_err(|_| ParseError {
                message: format!("register index '{digits}' is not a valid number"),
                src: make_src(source, name),
                span: make_span(source, line, col, text.len()),
            })?;
            let reg = u8::try_from(slot).map_err(|_| ParseError {
                message: format!("register index {slot} is out of range [0, 255]"),
                src: make_src(source, name),
                span: make_span(source, line, col, text.len()),
            })?;
            Ok(Operand::Register(reg))
        }
        Rule::integer => {
            let value = parse_integer(text, line, col, source, name)?;
            Ok(Operand::Integer(value))
        }
        Rule::label_id => Ok(Operand::LabelRef(text.to_string())),
        r => unreachable!("unexpected operand rule: {r:?}"),
    }
}

fn parse_integer(
    text: &str,
    line: usize,
    col: usize,
    source: &str,
    name: &str,
) -> Result<i64, ParseError> {
    let (neg, rest) = match text.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };

    let magnitude: u64 = if let Some(hex) = rest.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
    } else {
        rest.parse::<u64>()
    }
    .map_err(|_| ParseError {
        message: format!("invalid integer literal '{text}'"),
        src: make_src(source, name),
        span: make_span(source, line, col, text.len()),
    })?;

    if neg {
        // -magnitude must fit in i64: magnitude <= 2^63
        if magnitude > (i64::MAX as u64) + 1 {
            return Err(ParseError {
                message: format!("integer literal '{text}' underflows i64"),
                src: make_src(source, name),
                span: make_span(source, line, col, text.len()),
            });
        }
        Ok(-(magnitude as i64))
    } else {
        i64::try_from(magnitude).map_err(|_| ParseError {
            message: format!("integer literal '{text}' overflows i64"),
            src: make_src(source, name),
            span: make_span(source, line, col, text.len()),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Operand;

    fn instr(lines: &[AsmLine]) -> &ParsedInstr {
        for l in lines {
            if let AsmLine::Instruction(i) = l {
                return i;
            }
        }
        panic!("no instruction found")
    }

    fn parse_test(src: &str) -> Result<Vec<AsmLine>, ParseError> {
        parse(src, "<test>")
    }

    #[test]
    fn parse_nop() {
        let lines = parse_test("NOP").unwrap();
        assert_eq!(lines.len(), 1);
        let i = instr(&lines);
        assert_eq!(i.mnemonic, "NOP");
        assert!(i.operands.is_empty());
    }

    #[test]
    fn parse_push_positive() {
        let lines = parse_test("PUSH 42").unwrap();
        let i = instr(&lines);
        assert_eq!(i.mnemonic, "PUSH");
        assert_eq!(i.operands, [Operand::Integer(42)]);
    }

    #[test]
    fn parse_push_negative() {
        let lines = parse_test("PUSH -99").unwrap();
        let i = instr(&lines);
        assert_eq!(i.operands, [Operand::Integer(-99)]);
    }

    #[test]
    fn parse_push_hex() {
        let lines = parse_test("PUSH 0xFF").unwrap();
        let i = instr(&lines);
        assert_eq!(i.operands, [Operand::Integer(255)]);
    }

    #[test]
    fn parse_load_register() {
        let lines = parse_test("LOAD r3").unwrap();
        let i = instr(&lines);
        assert_eq!(i.mnemonic, "LOAD");
        assert_eq!(i.operands, [Operand::Register(3)]);
    }

    #[test]
    fn parse_energy_two_registers() {
        let lines = parse_test("ENERGY r0 r1").unwrap();
        let i = instr(&lines);
        assert_eq!(i.mnemonic, "ENERGY");
        assert_eq!(i.operands, [Operand::Register(0), Operand::Register(1)]);
    }

    #[test]
    fn parse_jump_label_ref() {
        let lines = parse_test("JUMP loop_top").unwrap();
        let i = instr(&lines);
        assert_eq!(i.operands, [Operand::LabelRef("loop_top".to_string())]);
    }

    #[test]
    fn parse_jump_integer_offset() {
        let lines = parse_test("JUMP -10").unwrap();
        let i = instr(&lines);
        assert_eq!(i.operands, [Operand::Integer(-10)]);
    }

    #[test]
    fn parse_label_def() {
        let lines = parse_test("loop:").unwrap();
        assert_eq!(lines.len(), 1);
        assert!(matches!(&lines[0], AsmLine::LabelDef { name, .. } if name == "loop"));
    }

    #[test]
    fn parse_label_and_instruction_same_line() {
        let lines = parse_test("start: NOP").unwrap();
        assert_eq!(lines.len(), 2);
        assert!(matches!(&lines[0], AsmLine::LabelDef { name, .. } if name == "start"));
        let i = instr(&lines);
        assert_eq!(i.mnemonic, "NOP");
    }

    #[test]
    fn parse_multiline_program() {
        let src = "PUSH 1\nPUSH 2\nADD\nHALT";
        let lines = parse_test(src).unwrap();
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn parse_comment_only_line_ignored() {
        let lines = parse_test("; this is a comment\nHALT").unwrap();
        assert_eq!(lines.len(), 1);
        let i = instr(&lines);
        assert_eq!(i.mnemonic, "HALT");
    }

    #[test]
    fn parse_inline_comment() {
        let lines = parse_test("NOP ; do nothing").unwrap();
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn parse_blank_lines_ignored() {
        let src = "\nNOP\n\nHALT\n";
        let lines = parse_test(src).unwrap();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn parse_register_out_of_range() {
        assert!(parse_test("LOAD r256").is_err());
    }

    #[test]
    fn parse_invalid_syntax_returns_error() {
        assert!(parse_test("@@@").is_err());
    }

    #[test]
    fn parse_vecpush_instruction() {
        let lines = parse_test("VECPUSH r5").unwrap();
        let i = instr(&lines);
        assert_eq!(i.mnemonic, "VECPUSH");
        assert_eq!(i.operands, [Operand::Register(5)]);
    }
}
