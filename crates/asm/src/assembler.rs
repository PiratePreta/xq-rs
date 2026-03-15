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

//! Assembler: converts a parsed [`AsmLine`] list into a binary bytecode
//! buffer.
//!
//! Delegates label resolution and `JUMP`/`JUMPI` fixups to
//! [`InstructionBuilder`]. Instructions are emitted in a single pass;
//! label-based jumps register fixups that [`InstructionBuilder::build`]
//! resolves at the end, supporting both forward and backward references.
//!
//! `source` and `name` are used solely for diagnostic output: they are
//! embedded in any [`AssembleError`] so that miette can render a source
//! snippet with a caret pointing at the failing token.
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_asm::assembler::assemble;
//! use aglais_xqvm_asm::ast::{AsmLine, Operand, ParsedInstr};
//!
//! let src = "PUSH 0\nHALT";
//! let lines = vec![
//!     AsmLine::Instruction(ParsedInstr {
//!         mnemonic: "PUSH".to_string(),
//!         operands: vec![Operand::Integer(0)],
//!         line: 1, col: 1,
//!     }),
//!     AsmLine::Instruction(ParsedInstr {
//!         mnemonic: "HALT".to_string(),
//!         operands: vec![],
//!         line: 2, col: 1,
//!     }),
//! ];
//! let bytes = assemble(&lines, src, "<test>").unwrap();
//! assert_eq!(bytes[0], 0x10); // PUSH opcode
//! assert_eq!(*bytes.last().unwrap(), 0x0F); // HALT opcode
//! ```

// AssembleError carries a NamedSource<Arc<str>> in every variant so that
// miette can render source snippets.  The extra size is acceptable because
// error paths in an assembler are not performance-critical.
#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use aglais_xqvm_bytecode::builder::InstructionBuilder;
use aglais_xqvm_bytecode::builder::LabelId;
use aglais_xqvm_bytecode::types::{Instruction, Register};
use aglais_xqvm_bytecode::{builder, opcodes};

use crate::ast::{AsmLine, Operand, ParsedInstr};
use crate::error::{AssembleError, make_span, make_src};

/// Maps a label name to its [`LabelId`] and the source location where it was
/// first defined (`None` if only seen as a forward reference so far).
type LabelMap = HashMap<String, (LabelId, Option<(usize, usize)>)>;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Assemble a list of parsed lines into a binary bytecode buffer.
///
/// `source` and `name` are used solely for diagnostic output: they are
/// embedded in any [`AssembleError`] so that miette can render a source
/// snippet with a caret pointing at the failing token.
///
/// Delegates to [`InstructionBuilder`] for label resolution and
/// `JUMP`/`JUMPI` fixups. Both forward and backward label references are
/// supported.
///
/// # Errors
///
/// - [`AssembleError::UnknownMnemonic`] -- unrecognised mnemonic.
/// - [`AssembleError::WrongOperandCount`] -- wrong number of operands.
/// - [`AssembleError::WrongOperandKind`] -- operand of wrong kind.
/// - [`AssembleError::RegisterOutOfRange`] -- register slot > 255.
/// - [`AssembleError::IntegerOutOfRange`] -- integer does not fit target type.
/// - [`AssembleError::UndefinedLabel`] -- label referenced but not defined.
/// - [`AssembleError::DuplicateLabel`] -- label defined more than once.
/// - [`AssembleError::JumpOffsetOverflow`] -- jump distance exceeds `i16`.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_asm::parser::parse;
/// use aglais_xqvm_asm::assembler::assemble;
///
/// let src = "PUSH 5\nPUSH 3\nADD\nHALT";
/// let lines = parse(src, "<test>").unwrap();
/// let bytes = assemble(&lines, src, "<test>").unwrap();
/// assert!(!bytes.is_empty());
/// ```
pub fn assemble(lines: &[AsmLine], source: &str, name: &str) -> Result<Vec<u8>, AssembleError> {
    let mut b = InstructionBuilder::new();
    // label name -> (LabelId, first definition location)
    // location is None when the label was first seen via a forward reference.
    let mut label_map: LabelMap = HashMap::new();
    // label names in creation order: label_names[raw_id] == name
    let mut label_names: Vec<String> = Vec::new();
    // first jump reference per label name: used for error span reporting
    let mut first_ref: HashMap<String, (usize, usize)> = HashMap::new();

    for line in lines {
        match line {
            AsmLine::LabelDef {
                name: label_name,
                line: def_line,
                col: def_col,
            } => {
                let id = match label_map.entry(label_name.clone()) {
                    Entry::Occupied(e) => {
                        let (id, placed_at) = e.into_mut();
                        if let Some((prev_line, prev_col)) = *placed_at {
                            return Err(AssembleError::DuplicateLabel {
                                label: label_name.clone(),
                                src: make_src(source, name),
                                span: make_span(source, prev_line, prev_col, label_name.len()),
                            });
                        }
                        *placed_at = Some((*def_line, *def_col));
                        *id
                    }
                    Entry::Vacant(e) => {
                        label_names.push(label_name.clone());
                        let id = b.label();
                        e.insert((id, Some((*def_line, *def_col))));
                        id
                    }
                };
                b.place(id);
            }
            AsmLine::Instruction(instr) => match instr.mnemonic.as_str() {
                "JUMP" | "JUMPI" => {
                    assemble_jump(
                        instr,
                        &mut b,
                        &mut label_map,
                        &mut label_names,
                        &mut first_ref,
                        source,
                        name,
                    )?;
                }
                _ => {
                    b.emit(build_instr(instr, source, name)?);
                }
            },
        }
    }

    b.build()
        .map(|program| program.code().to_vec())
        .map_err(|e| convert_build_error(e, &label_names, &first_ref, source, name))
}

// ---------------------------------------------------------------------------
// JUMP / JUMPI
// ---------------------------------------------------------------------------

fn assemble_jump(
    instr: &ParsedInstr,
    b: &mut InstructionBuilder,
    label_map: &mut LabelMap,
    label_names: &mut Vec<String>,
    first_ref: &mut HashMap<String, (usize, usize)>,
    source: &str,
    name: &str,
) -> Result<(), AssembleError> {
    let mnem_span = make_span(source, instr.line, instr.col, instr.mnemonic.len());

    if instr.operands.len() != 1 {
        return Err(AssembleError::WrongOperandCount {
            mnemonic: instr.mnemonic.clone(),
            expected: 1,
            got: instr.operands.len(),
            src: make_src(source, name),
            span: mnem_span,
        });
    }

    match &instr.operands[0] {
        Operand::LabelRef(label) => {
            let id = match label_map.entry(label.clone()) {
                Entry::Occupied(e) => e.get().0,
                Entry::Vacant(e) => {
                    label_names.push(label.clone());
                    let id = b.label();
                    e.insert((id, None));
                    id
                }
            };
            first_ref
                .entry(label.clone())
                .or_insert((instr.line, instr.col));
            match instr.mnemonic.as_str() {
                "JUMPI" => b.jump_if(id),
                _ => b.jump(id),
            };
        }
        Operand::Integer(n) => {
            let offset = i16::try_from(*n).map_err(|_| AssembleError::IntegerOutOfRange {
                value: *n,
                target_type: "i16",
                field: "offset".to_string(),
                mnemonic: instr.mnemonic.clone(),
                src: make_src(source, name),
                span: mnem_span,
            })?;
            match instr.mnemonic.as_str() {
                "JUMPI" => b.emit(Instruction::JumpI { offset }),
                _ => b.emit(Instruction::Jump { offset }),
            };
        }
        Operand::Register(_) => {
            return Err(AssembleError::WrongOperandKind {
                mnemonic: instr.mnemonic.clone(),
                field: "offset".to_string(),
                expected_kind: "integer or label reference".to_string(),
                src: make_src(source, name),
                span: mnem_span,
            });
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Convert InstructionBuilder errors to AssembleError
// ---------------------------------------------------------------------------

fn convert_build_error(
    e: builder::Error,
    label_names: &[String],
    first_ref: &HashMap<String, (usize, usize)>,
    source: &str,
    name: &str,
) -> AssembleError {
    match e {
        builder::Error::UnplacedLabel { id } => {
            let label = label_names.get(id).cloned().unwrap_or_default();
            let (line, col) = first_ref.get(&label).copied().unwrap_or((1, 1));
            AssembleError::UndefinedLabel {
                label: label.clone(),
                src: make_src(source, name),
                span: make_span(source, line, col, label.len()),
            }
        }
        builder::Error::OffsetOverflow {
            label: id, delta, ..
        } => {
            let label = label_names.get(id).cloned().unwrap_or_default();
            let (line, col) = first_ref.get(&label).copied().unwrap_or((1, 1));
            AssembleError::JumpOffsetOverflow {
                label: label.clone(),
                delta,
                src: make_src(source, name),
                span: make_span(source, line, col, label.len()),
            }
        }
        builder::Error::PoolOverflow => AssembleError::PoolOverflow {
            src: make_src(source, name),
        },
    }
}

// ---------------------------------------------------------------------------
// Generic instruction assembly via opcodes! x-macro
// ---------------------------------------------------------------------------

/// Trait for converting a parsed [`Operand`] into a concrete field type.
trait FromOperand: Sized {
    fn from_operand(
        op: &Operand,
        field: &str,
        mnemonic: &str,
        line: usize,
        col: usize,
        source: &str,
        name: &str,
    ) -> Result<Self, AssembleError>;
}

impl FromOperand for Register {
    fn from_operand(
        op: &Operand,
        field: &str,
        mnemonic: &str,
        line: usize,
        col: usize,
        source: &str,
        name: &str,
    ) -> Result<Self, AssembleError> {
        match op {
            Operand::Register(n) => Ok(Self(*n)),
            _ => Err(AssembleError::WrongOperandKind {
                mnemonic: mnemonic.to_string(),
                field: field.to_string(),
                expected_kind: "register (e.g. r0)".to_string(),
                src: make_src(source, name),
                span: make_span(source, line, col, mnemonic.len()),
            }),
        }
    }
}

impl FromOperand for i64 {
    fn from_operand(
        op: &Operand,
        field: &str,
        mnemonic: &str,
        line: usize,
        col: usize,
        source: &str,
        name: &str,
    ) -> Result<Self, AssembleError> {
        match op {
            Operand::Integer(n) => Ok(*n),
            _ => Err(AssembleError::WrongOperandKind {
                mnemonic: mnemonic.to_string(),
                field: field.to_string(),
                expected_kind: "integer literal".to_string(),
                src: make_src(source, name),
                span: make_span(source, line, col, mnemonic.len()),
            }),
        }
    }
}

/// `u16` is used for `PUSHC` pool indices.
impl FromOperand for u16 {
    fn from_operand(
        op: &Operand,
        field: &str,
        mnemonic: &str,
        line: usize,
        col: usize,
        source: &str,
        name: &str,
    ) -> Result<Self, AssembleError> {
        match op {
            Operand::Integer(n) => {
                Self::try_from(*n).map_err(|_| AssembleError::IntegerOutOfRange {
                    value: *n,
                    target_type: "u16",
                    field: field.to_string(),
                    mnemonic: mnemonic.to_string(),
                    src: make_src(source, name),
                    span: make_span(source, line, col, mnemonic.len()),
                })
            }
            _ => Err(AssembleError::WrongOperandKind {
                mnemonic: mnemonic.to_string(),
                field: field.to_string(),
                expected_kind: "integer literal".to_string(),
                src: make_src(source, name),
                span: make_span(source, line, col, mnemonic.len()),
            }),
        }
    }
}

/// `i16` is used only for `JUMP`/`JUMPI` offsets, which are handled
/// separately. This impl covers hypothetical non-jump opcodes that take an
/// `i16` field (currently none).
impl FromOperand for i16 {
    fn from_operand(
        op: &Operand,
        field: &str,
        mnemonic: &str,
        line: usize,
        col: usize,
        source: &str,
        name: &str,
    ) -> Result<Self, AssembleError> {
        match op {
            Operand::Integer(n) => {
                Self::try_from(*n).map_err(|_| AssembleError::IntegerOutOfRange {
                    value: *n,
                    target_type: "i16",
                    field: field.to_string(),
                    mnemonic: mnemonic.to_string(),
                    src: make_src(source, name),
                    span: make_span(source, line, col, mnemonic.len()),
                })
            }
            _ => Err(AssembleError::WrongOperandKind {
                mnemonic: mnemonic.to_string(),
                field: field.to_string(),
                expected_kind: "integer literal".to_string(),
                src: make_src(source, name),
                span: make_span(source, line, col, mnemonic.len()),
            }),
        }
    }
}

/// Generate `fn build_instr(instr, source, name) -> Result<Instruction, AssembleError>`
/// using the full opcode table. `JUMP` and `JUMPI` arms are unreachable at
/// runtime because the caller routes them to `assemble_jump` first.
macro_rules! impl_build_instr {
    ( $( ($code:literal, $variant:ident, $mnem:literal, $doc:literal,
          {$($fname:ident: $ftype:ty),*}) ),* $(,)? ) => {

        fn build_instr(
            instr: &ParsedInstr,
            source: &str,
            name: &str,
        ) -> Result<Instruction, AssembleError> {
            match instr.mnemonic.as_str() {
                $(
                    $mnem => {
                        const EXPECTED: usize = 0usize
                            $( + { let _ = stringify!($fname); 1 })*;

                        if instr.operands.len() != EXPECTED {
                            return Err(AssembleError::WrongOperandCount {
                                mnemonic: instr.mnemonic.clone(),
                                expected: EXPECTED,
                                got: instr.operands.len(),
                                src: make_src(source, name),
                                span: make_span(
                                    source, instr.line, instr.col,
                                    instr.mnemonic.len(),
                                ),
                            });
                        }

                        let mut _iter = instr.operands.iter();
                        $(
                            // SAFETY: operand count was verified to equal EXPECTED above.
                            let $fname = <$ftype as FromOperand>::from_operand(
                                _iter.next().unwrap_or_else(|| unreachable!()),
                                stringify!($fname),
                                &instr.mnemonic,
                                instr.line,
                                instr.col,
                                source,
                                name,
                            )?;
                        )*

                        Ok(Instruction::$variant { $($fname,)* })
                    }
                )*
                _ => Err(AssembleError::UnknownMnemonic {
                    mnemonic: instr.mnemonic.clone(),
                    src: make_src(source, name),
                    span: make_span(source, instr.line, instr.col, instr.mnemonic.len()),
                }),
            }
        }
    };
}

opcodes!(impl_build_instr);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use aglais_xqvm_bytecode::stream::InstructionStream;
    use aglais_xqvm_bytecode::types::Instruction;

    fn decode_all(buf: &[u8]) -> Vec<Instruction> {
        InstructionStream::new(buf).map(|r| r.unwrap().2).collect()
    }

    fn asm(src: &str) -> Vec<u8> {
        let lines = parse(src, "<test>").unwrap();
        assemble(&lines, src, "<test>").unwrap()
    }

    #[test]
    fn halt_is_one_byte() {
        assert_eq!(asm("HALT"), [0x0F]);
    }

    #[test]
    fn nop_is_one_byte() {
        assert_eq!(asm("NOP"), [0x00]);
    }

    #[test]
    fn push_zero() {
        // opcode 0x10, i64(0) in BE = 8 zero bytes
        assert_eq!(
            asm("PUSH 0"),
            [0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        );
    }

    #[test]
    fn push_negative() {
        // i64(-1) in BE = [0xFF; 8]
        assert_eq!(
            asm("PUSH -1"),
            [0x10, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
        );
    }

    #[test]
    fn load_register() {
        assert_eq!(asm("LOAD r3"), [0x14, 0x03]);
    }

    #[test]
    fn energy_two_registers() {
        assert_eq!(asm("ENERGY r2 r3"), [0x7F, 0x02, 0x03]);
    }

    #[test]
    fn simple_program_roundtrip() {
        let src = "PUSH 5\nPUSH 3\nADD\nHALT";
        let buf = asm(src);
        let instrs = decode_all(&buf);
        assert_eq!(instrs[0], Instruction::Push { imm: 5 });
        assert_eq!(instrs[1], Instruction::Push { imm: 3 });
        assert_eq!(instrs[2], Instruction::Add {});
        assert_eq!(instrs[3], Instruction::Halt {});
    }

    #[test]
    fn forward_jump_label() {
        // JUMP done (3 bytes at site 0)
        // NOP       (1 byte  at site 3)
        // done:
        // HALT      (1 byte  at site 4)
        // => delta = 4 - 0 = 4
        let src = "JUMP done\nNOP\ndone:\nHALT";
        let buf = asm(src);
        let instrs = decode_all(&buf);
        assert_eq!(instrs[0], Instruction::Jump { offset: 4 });
        assert_eq!(instrs[1], Instruction::Nop {});
        assert_eq!(instrs[2], Instruction::Halt {});
    }

    #[test]
    fn backward_jumpi_label() {
        // top:
        // PUSH -1 (9 bytes at 0)
        // ADD     (1 byte  at 9)
        // DUPL    (1 byte  at 10)
        // JUMPI top (3 bytes at 11)
        // => delta = 0 - 11 = -11
        let src = "top:\nPUSH -1\nADD\nDUPL\nJUMPI top";
        let buf = asm(src);
        let instrs = decode_all(&buf);
        assert_eq!(instrs.last().unwrap(), &Instruction::JumpI { offset: -11 });
    }

    #[test]
    fn jump_raw_integer_offset() {
        let buf = asm("JUMP 3");
        let instrs = decode_all(&buf);
        assert_eq!(instrs[0], Instruction::Jump { offset: 3 });
    }

    #[test]
    fn unknown_mnemonic_error() {
        let src = "FOOBAR";
        let lines = parse(src, "<test>").unwrap();
        assert!(assemble(&lines, src, "<test>").is_err());
    }

    #[test]
    fn wrong_operand_count_error() {
        let src = "HALT r0";
        let lines = parse(src, "<test>").unwrap();
        assert!(assemble(&lines, src, "<test>").is_err());
    }

    #[test]
    fn wrong_operand_kind_error() {
        // LOAD expects a register, not an integer
        let src = "LOAD 42";
        let lines = parse(src, "<test>").unwrap();
        assert!(assemble(&lines, src, "<test>").is_err());
    }

    #[test]
    fn undefined_label_error() {
        let src = "JUMP nowhere";
        let lines = parse(src, "<test>").unwrap();
        assert!(assemble(&lines, src, "<test>").is_err());
    }

    #[test]
    fn duplicate_label_error() {
        let src = "top:\nNOP\ntop:\nHALT";
        let lines = parse(src, "<test>").unwrap();
        assert!(assemble(&lines, src, "<test>").is_err());
    }

    #[test]
    fn duplicate_label_reports_first_definition_location() {
        let src = "top:\nNOP\ntop:\nHALT";
        let lines = parse(src, "<test>").unwrap();
        let err = assemble(&lines, src, "<test>").unwrap_err();
        assert!(matches!(
            err,
            AssembleError::DuplicateLabel { ref label, .. } if label == "top"
        ));
    }

    #[test]
    fn push_hex_literal() {
        let buf = asm("PUSH 0xFF");
        let instrs = decode_all(&buf);
        assert_eq!(instrs[0], Instruction::Push { imm: 255 });
    }

    #[test]
    fn all_zero_arg_instructions_assemble() {
        // Spot-check a handful of zero-operand mnemonics.
        for mnem in &["NOP", "HALT", "ADD", "SUB", "MUL", "DIV", "NOT", "AND"] {
            let buf = asm(mnem);
            assert_eq!(buf.len(), 1, "expected 1 byte for {mnem}");
        }
    }
}
