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

//! Assembler: converts a parsed [`AsmLine`] list into a [`Program`].
//!
//! Delegates label resolution and `JUMP`/`JUMPI` fixups to
//! [`InstructionBuilder`]. Instructions are emitted in a single pass;
//! label-based jumps register fixups that [`InstructionBuilder::build`]
//! resolves at the end, supporting both forward and backward references.
//!
//! `PUSH` is handled specially: the assembly operand is an integer constant.
//! The assembler calls [`InstructionBuilder::push`] which selects the minimal
//! inline-constant variant (`PUSHC_0`..`PUSHC_8`) automatically.
//!
//! `source` and `name` are used solely for diagnostic output: they are
//! embedded in any [`AssembleError`] so that miette can render a source
//! snippet with a caret pointing at the failing token.
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_asm::{assemble, AsmLine, Operand, ParsedInstr};
//!
//! let src = "PUSH 0\nHALT";
//! let lines = vec![
//!     AsmLine::Instruction(ParsedInstr {
//!         mnemonic: "PUSH".to_string(),
//!         operands: vec![Operand::Integer(0)],
//!         offset: 0,
//!     }),
//!     AsmLine::Instruction(ParsedInstr {
//!         mnemonic: "HALT".to_string(),
//!         operands: vec![],
//!         offset: 7,
//!     }),
//! ];
//! let program = assemble(&lines, src, "<test>").unwrap();
//! assert_eq!(program.code()[0], 0x10); // PUSHC_0 opcode
//! assert_eq!(*program.code().last().unwrap(), 0x0F); // HALT opcode
//! ```

// AssembleError carries a NamedSource<Arc<str>> in every variant so that
// miette can render source snippets.  The extra size is acceptable because
// error paths in an assembler are not performance-critical.
#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use aglais_xqvm_bytecode::error::BuilderError;
use aglais_xqvm_bytecode::{Instruction, InstructionBuilder, LabelId, Program, Register, opcodes};

use crate::ast::{AsmLine, Operand, ParsedInstr};
use crate::error::{AssembleError, Source, make_span, make_src};

/// Maps a label name to its [`LabelId`] and the source location where it was
/// first defined (`None` if only seen as a forward reference so far).
type LabelMap = HashMap<String, (LabelId, Option<usize>)>;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Assemble a list of parsed lines into a [`Program`].
///
/// `source` and `name` are used solely for diagnostic output: they are
/// embedded in any [`AssembleError`] so that miette can render a source
/// snippet with a caret pointing at the failing token.
///
/// Delegates to [`InstructionBuilder`] for label resolution and
/// `JUMP`/`JUMPI` fixups. Both forward and backward label references are
/// supported.
///
/// `PUSHC <imm>` is handled specially: the integer operand is the constant
/// value to intern, not a pool index. The pool index is allocated
/// automatically and deduplicated.
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
/// use aglais_xqvm_asm::{parse, assemble};
///
/// let src = "PUSH 5\nPUSH 3\nADD\nHALT";
/// let lines = parse(src, "<test>").unwrap();
/// let program = assemble(&lines, src, "<test>").unwrap();
/// assert!(!program.code().is_empty());
/// ```
pub fn assemble(lines: &[AsmLine], source: &str, name: &str) -> Result<Program, AssembleError> {
    let src = Source { text: source, name };
    let mut b = InstructionBuilder::new();
    // label name -> (LabelId, first definition location)
    // location is None when the label was first seen via a forward reference.
    let mut label_map: LabelMap = HashMap::new();
    // label names in creation order: label_names[raw_id] == name
    let mut label_names: Vec<String> = Vec::new();
    // first jump reference per label name: used for error span reporting
    let mut first_ref: HashMap<String, usize> = HashMap::new();

    for line in lines {
        match line {
            AsmLine::LabelDef {
                name: label_name,
                offset: def_offset,
            } => {
                let id = match label_map.entry(label_name.clone()) {
                    Entry::Occupied(e) => {
                        let (id, placed_at) = e.into_mut();
                        if let Some(prev_offset) = *placed_at {
                            return Err(AssembleError::DuplicateLabel {
                                label: label_name.clone(),
                                src: make_src(src),
                                span: make_span(prev_offset, label_name.len()),
                            });
                        }
                        *placed_at = Some(*def_offset);
                        *id
                    }
                    Entry::Vacant(e) => {
                        label_names.push(label_name.clone());
                        let id = b.label();
                        let _ = e.insert((id, Some(*def_offset)));
                        id
                    }
                };
                let _ = b.place(id);
            }
            AsmLine::Instruction(instr) => match instr.mnemonic.as_str() {
                "JUMP" | "JUMPI" => {
                    assemble_jump(
                        instr,
                        &mut b,
                        &mut label_map,
                        &mut label_names,
                        &mut first_ref,
                        src,
                    )?;
                }
                "PUSH" | "PUSHC" => {
                    assemble_push(instr, &mut b, src)?;
                }
                _ => {
                    let _ = b.emit(build_instr(instr, src)?);
                }
            },
        }
    }

    b.build()
        .map_err(|e| convert_build_error(e, &label_names, &first_ref, src))
}

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

/// Return `Err(WrongOperandCount)` when `instr.operands.len() != expected`.
fn check_operand_count(
    instr: &ParsedInstr,
    expected: usize,
    src: Source<'_>,
) -> Result<(), AssembleError> {
    if instr.operands.len() != expected {
        return Err(AssembleError::WrongOperandCount {
            mnemonic: instr.mnemonic.clone(),
            expected,
            got: instr.operands.len(),
            src: make_src(src),
            span: make_span(instr.offset, instr.mnemonic.len()),
        });
    }
    Ok(())
}

/// Build a `WrongOperandKind` error pointing at the mnemonic token.
fn err_wrong_kind(
    instr: &ParsedInstr,
    field: &str,
    expected_kind: &str,
    src: Source<'_>,
) -> AssembleError {
    AssembleError::WrongOperandKind {
        mnemonic: instr.mnemonic.clone(),
        field: field.to_string(),
        expected_kind: expected_kind.to_string(),
        src: make_src(src),
        span: make_span(instr.offset, instr.mnemonic.len()),
    }
}

// ---------------------------------------------------------------------------
// JUMP / JUMPI
// ---------------------------------------------------------------------------

fn assemble_jump(
    instr: &ParsedInstr,
    b: &mut InstructionBuilder,
    label_map: &mut LabelMap,
    label_names: &mut Vec<String>,
    first_ref: &mut HashMap<String, usize>,
    src: Source<'_>,
) -> Result<(), AssembleError> {
    let mnem_span = make_span(instr.offset, instr.mnemonic.len());

    check_operand_count(instr, 1, src)?;

    match instr
        .operands
        .first()
        .unwrap_or_else(|| unreachable!("check_operand_count ensures len == 1"))
    {
        Operand::LabelRef(label) => {
            let id = match label_map.entry(label.clone()) {
                Entry::Occupied(e) => e.get().0,
                Entry::Vacant(e) => {
                    label_names.push(label.clone());
                    let id = b.label();
                    let _ = e.insert((id, None));
                    id
                }
            };
            let _ = first_ref.entry(label.clone()).or_insert(instr.offset);
            let _ = match instr.mnemonic.as_str() {
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
                src: make_src(src),
                span: mnem_span,
            })?;
            let _ = match instr.mnemonic.as_str() {
                "JUMPI" => b.emit(Instruction::JumpI { offset }),
                _ => b.emit(Instruction::Jump { offset }),
            };
        }
        Operand::Register(_) => {
            return Err(err_wrong_kind(
                instr,
                "offset",
                "integer or label reference",
                src,
            ));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// PUSH / PUSHC
// ---------------------------------------------------------------------------

/// Handle `PUSH <imm>` and `PUSHC <imm>`: encode the integer constant inline
/// using the minimal `PUSHC_N` encoding chosen by [`InstructionBuilder::push`].
fn assemble_push(
    instr: &ParsedInstr,
    b: &mut InstructionBuilder,
    src: Source<'_>,
) -> Result<(), AssembleError> {
    check_operand_count(instr, 1, src)?;

    match instr
        .operands
        .first()
        .unwrap_or_else(|| unreachable!("check_operand_count ensures len == 1"))
    {
        Operand::Integer(imm) => {
            let _ = b.push(*imm);
        }
        _ => {
            return Err(err_wrong_kind(instr, "imm", "integer literal", src));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Convert InstructionBuilder errors to AssembleError
// ---------------------------------------------------------------------------

fn convert_build_error(
    e: BuilderError,
    label_names: &[String],
    first_ref: &HashMap<String, usize>,
    src: Source<'_>,
) -> AssembleError {
    match e {
        BuilderError::UnplacedLabel { id } => {
            let label = label_names.get(id).cloned().unwrap_or_default();
            let offset = first_ref.get(&label).copied().unwrap_or(0);
            AssembleError::UndefinedLabel {
                label: label.clone(),
                src: make_src(src),
                span: make_span(offset, label.len()),
            }
        }
        BuilderError::OffsetOverflow {
            label: id, delta, ..
        } => {
            let label = label_names.get(id).cloned().unwrap_or_default();
            let offset = first_ref.get(&label).copied().unwrap_or(0);
            AssembleError::JumpOffsetOverflow {
                label: label.clone(),
                delta,
                src: make_src(src),
                span: make_span(offset, label.len()),
            }
        }
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
        offset: usize,
        src: Source<'_>,
    ) -> Result<Self, AssembleError>;
}

impl FromOperand for Register {
    fn from_operand(
        op: &Operand,
        field: &str,
        mnemonic: &str,
        offset: usize,
        src: Source<'_>,
    ) -> Result<Self, AssembleError> {
        match op {
            Operand::Register(n) => Ok(Self(*n)),
            _ => Err(AssembleError::WrongOperandKind {
                mnemonic: mnemonic.to_string(),
                field: field.to_string(),
                expected_kind: "register (e.g. r0)".to_string(),
                src: make_src(src),
                span: make_span(offset, mnemonic.len()),
            }),
        }
    }
}

impl FromOperand for i64 {
    fn from_operand(
        op: &Operand,
        field: &str,
        mnemonic: &str,
        offset: usize,
        src: Source<'_>,
    ) -> Result<Self, AssembleError> {
        match op {
            Operand::Integer(n) => Ok(*n),
            _ => Err(AssembleError::WrongOperandKind {
                mnemonic: mnemonic.to_string(),
                field: field.to_string(),
                expected_kind: "integer literal".to_string(),
                src: make_src(src),
                span: make_span(offset, mnemonic.len()),
            }),
        }
    }
}

/// `[u8; N]` fields appear on `PUSHC_1`..`PUSHC_8` instructions.
///
/// These mnemonics are not user-facing (assembly uses `PUSH`/`PUSHC` which are
/// handled by `assemble_push` before this path is reached), so this impl is
/// only present to satisfy the compiler for the `impl_build_instr` macro arms.
macro_rules! impl_from_operand_byte_array {
    ($($n:literal),+) => {
        $(
            impl FromOperand for [u8; $n] {
                fn from_operand(
                    _op: &Operand,
                    field: &str,
                    mnemonic: &str,
                    offset: usize,
                    src: Source<'_>,
                ) -> Result<Self, AssembleError> {
                    Err(AssembleError::UnknownMnemonic {
                        mnemonic: mnemonic.to_string(),
                        src: make_src(src),
                        span: make_span(offset, field.len()),
                    })
                }
            }
        )+
    };
}

impl_from_operand_byte_array!(1, 2, 3, 4, 5, 6, 7, 8);

/// `i16` is used only for `JUMP`/`JUMPI` offsets, which are handled
/// separately. This impl covers hypothetical non-jump opcodes that take an
/// `i16` field (currently none).
impl FromOperand for i16 {
    fn from_operand(
        op: &Operand,
        field: &str,
        mnemonic: &str,
        offset: usize,
        src: Source<'_>,
    ) -> Result<Self, AssembleError> {
        match op {
            Operand::Integer(n) => {
                Self::try_from(*n).map_err(|_| AssembleError::IntegerOutOfRange {
                    value: *n,
                    target_type: "i16",
                    field: field.to_string(),
                    mnemonic: mnemonic.to_string(),
                    src: make_src(src),
                    span: make_span(offset, mnemonic.len()),
                })
            }
            _ => Err(AssembleError::WrongOperandKind {
                mnemonic: mnemonic.to_string(),
                field: field.to_string(),
                expected_kind: "integer literal".to_string(),
                src: make_src(src),
                span: make_span(offset, mnemonic.len()),
            }),
        }
    }
}

/// Generate `fn build_instr(instr, src) -> Result<Instruction, AssembleError>`
/// using the full opcode table. `JUMP` and `JUMPI` arms are unreachable at
/// runtime because the caller routes them to `assemble_jump` first.
macro_rules! impl_build_instr {
    ( $( ($code:literal, $variant:ident, $mnem:literal, $doc:literal,
          {$($fname:ident: $ftype:ty),*}) ),* $(,)? ) => {

        fn build_instr(
            instr: &ParsedInstr,
            src: Source<'_>,
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
                                src: make_src(src),
                                span: make_span(instr.offset, instr.mnemonic.len()),
                            });
                        }

                        let mut _iter = instr.operands.iter();
                        $(
                            // SAFETY: operand count was verified to equal EXPECTED above.
                            let $fname = <$ftype as FromOperand>::from_operand(
                                _iter.next().unwrap_or_else(|| unreachable!()),
                                stringify!($fname),
                                &instr.mnemonic,
                                instr.offset,
                                src,
                            )?;
                        )*

                        Ok(Instruction::$variant { $($fname,)* })
                    }
                )*
                _ => Err(AssembleError::UnknownMnemonic {
                    mnemonic: instr.mnemonic.clone(),
                    src: make_src(src),
                    span: make_span(instr.offset, instr.mnemonic.len()),
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
#[allow(clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::parser::parse;
    use aglais_xqvm_bytecode::{Instruction, InstructionStream};

    fn decode_all(buf: &[u8]) -> Vec<Instruction> {
        InstructionStream::new(buf).map(|r| r.unwrap().2).collect()
    }

    fn asm(src: &str) -> Vec<u8> {
        let lines = parse(src, "<test>").unwrap();
        assemble(&lines, src, "<test>").unwrap().code().to_vec()
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
        // 0 encodes as PUSHC_0 (opcode 0x10, no payload)
        assert_eq!(asm("PUSH 0"), [0x10]);
    }

    #[test]
    fn push_negative() {
        // -1 encodes as PUSHC_1 (opcode 0x18) with 1-byte payload 0xFF
        assert_eq!(asm("PUSH -1"), [0x18, 0xFF]);
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
        assert_eq!(instrs[0], Instruction::PushC1 { val: [5] });
        assert_eq!(instrs[1], Instruction::PushC1 { val: [3] });
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
        // PUSH -1   (2 bytes at 0: PUSHC_1 0xFF)
        // ADD       (1 byte  at 2)
        // DUPL      (1 byte  at 3)
        // JUMPI top (3 bytes at 4)
        // => delta = 0 - 4 = -4
        let src = "top:\nPUSH -1\nADD\nDUPL\nJUMPI top";
        let buf = asm(src);
        let instrs = decode_all(&buf);
        assert_eq!(instrs.last().unwrap(), &Instruction::JumpI { offset: -4 });
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
        // 255 fits in 2 bytes (needs sign bit), so PUSHC_2 { val: [0x00, 0xFF] }
        assert_eq!(instrs[0], Instruction::PushC2 { val: [0x00, 0xFF] });
    }

    #[test]
    fn all_zero_arg_instructions_assemble() {
        // Spot-check a handful of zero-operand mnemonics.
        for mnem in &["NOP", "HALT", "ADD", "SUB", "MUL", "DIV", "NOT", "AND"] {
            let buf = asm(mnem);
            assert_eq!(buf.len(), 1, "expected 1 byte for {mnem}");
        }
    }

    #[test]
    fn pushc_assembles_inline() {
        let src = "PUSHC 12345";
        let lines = parse(src, "<test>").unwrap();
        let program = assemble(&lines, src, "<test>").unwrap();
        // 12345 = 0x3039, fits in 2 bytes, so PUSHC_2 { val: [0x30, 0x39] }.
        let instrs = decode_all(program.code());
        assert_eq!(instrs.len(), 1);
        assert_eq!(instrs[0], Instruction::PushC2 { val: [0x30, 0x39] });
    }

    #[test]
    fn pushc_wrong_operand_count_error() {
        let src = "PUSHC";
        let lines = parse(src, "<test>").unwrap();
        let err = assemble(&lines, src, "<test>").unwrap_err();
        assert!(matches!(err, AssembleError::WrongOperandCount { .. }));
    }

    #[test]
    fn pushc_wrong_operand_kind_error() {
        let src = "PUSHC r0";
        let lines = parse(src, "<test>").unwrap();
        let err = assemble(&lines, src, "<test>").unwrap_err();
        assert!(matches!(err, AssembleError::WrongOperandKind { .. }));
    }
}
