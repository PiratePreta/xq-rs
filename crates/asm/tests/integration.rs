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

//! Integration tests for the XQVM assembler.
//!
//! These tests assemble complete programs and verify the decoded instruction
//! stream, exercising the full parse -> assemble -> decode pipeline.

#![allow(clippy::indexing_slicing)]

use aglais_xqvm_asm::assemble_source;
use aglais_xqvm_bytecode::{Instruction, InstructionStream, Register};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn decode_all(buf: &[u8]) -> Vec<Instruction> {
    InstructionStream::new(buf).map(|r| r.unwrap().2).collect()
}

fn asm(src: &str) -> Vec<Instruction> {
    let program = assemble_source(src).expect("assemble_source failed");
    decode_all(program.code())
}

// ---------------------------------------------------------------------------
// Minimal programs
// ---------------------------------------------------------------------------

#[test]
fn empty_program_produces_empty_buffer() {
    let prog = assemble_source("").unwrap();
    assert!(prog.code().is_empty());
}

#[test]
fn comment_only_produces_empty_buffer() {
    let prog = assemble_source("; nothing here\n; more comments").unwrap();
    assert!(prog.code().is_empty());
}

#[test]
fn single_halt() {
    let instrs = asm("HALT");
    assert_eq!(instrs, [Instruction::Halt {}]);
}

#[test]
fn push_and_halt() {
    let instrs = asm("PUSH 7\nHALT");
    assert_eq!(instrs[0], Instruction::PushC1 { val: [7] });
    assert_eq!(instrs[1], Instruction::Halt {});
}

// ---------------------------------------------------------------------------
// Arithmetic
// ---------------------------------------------------------------------------

#[test]
fn arithmetic_sequence() {
    let instrs = asm("PUSH 10\nPUSH 3\nSUB\nNEG\nHALT");
    assert_eq!(instrs[0], Instruction::PushC1 { val: [10] });
    assert_eq!(instrs[1], Instruction::PushC1 { val: [3] });
    assert_eq!(instrs[2], Instruction::Sub {});
    assert_eq!(instrs[3], Instruction::Neg {});
    assert_eq!(instrs[4], Instruction::Halt {});
}

#[test]
fn bitwise_instructions() {
    let instrs = asm("PUSH 0xFF\nPUSH 0x0F\nBAND\nHALT");
    assert_eq!(instrs[0], Instruction::PushC2 { val: [0x00, 0xFF] });
    assert_eq!(instrs[1], Instruction::PushC1 { val: [0x0F] });
    assert_eq!(instrs[2], Instruction::BAnd {});
}

// ---------------------------------------------------------------------------
// Registers
// ---------------------------------------------------------------------------

#[test]
fn stow_and_load_roundtrip() {
    let instrs = asm("PUSH 99\nSTOW r5\nLOAD r5\nHALT");
    assert_eq!(instrs[0], Instruction::PushC1 { val: [99] });
    assert_eq!(instrs[1], Instruction::Stow { reg: Register(5) });
    assert_eq!(instrs[2], Instruction::Load { reg: Register(5) });
}

#[test]
fn max_register_r255() {
    let instrs = asm("PUSH 1\nSTOW r255\nHALT");
    assert_eq!(instrs[1], Instruction::Stow { reg: Register(255) });
}

#[test]
fn energy_instruction() {
    let instrs = asm("ENERGY r0 r1");
    assert_eq!(
        instrs[0],
        Instruction::Energy {
            model: Register(0),
            sample: Register(1),
        }
    );
}

// ---------------------------------------------------------------------------
// Labels and jumps
// ---------------------------------------------------------------------------

#[test]
fn forward_jump_resolves() {
    // JUMP done  (3 bytes, site=0)
    // NOP        (1 byte,  site=3)
    // done:
    // HALT       (1 byte,  site=4)
    // delta = 4 - 0 = 4
    let instrs = asm("JUMP done\nNOP\ndone:\nHALT");
    assert_eq!(instrs[0], Instruction::Jump { offset: 4 });
    assert_eq!(instrs[1], Instruction::Nop {});
    assert_eq!(instrs[2], Instruction::Halt {});
}

#[test]
fn backward_jumpi_resolves() {
    // top:
    // PUSH -1   (2 bytes, site=0: PUSHC_1 0xFF)
    // ADD       (1 byte,  site=2)
    // DUPL      (1 byte,  site=3)
    // JUMPI top (3 bytes, site=4)
    // delta = 0 - 4 = -4
    let instrs = asm("top:\nPUSH -1\nADD\nDUPL\nJUMPI top");
    assert_eq!(instrs.last().unwrap(), &Instruction::JumpI { offset: -4 });
}

#[test]
fn label_on_same_line_as_instruction() {
    // start: NOP -- label is at byte 0, NOP follows
    // JUMP start -- delta = 0 - 1 = -1
    let instrs = asm("start: NOP\nJUMP start");
    assert_eq!(instrs[0], Instruction::Nop {});
    assert_eq!(instrs[1], Instruction::Jump { offset: -1 });
}

#[test]
fn multiple_labels_and_jumps() {
    let src = "
        PUSH 0
        JUMPI done
        PUSH 42
    done:
        HALT
    ";
    let instrs = asm(src);
    assert_eq!(instrs[0], Instruction::PushC0 {});
    assert!(matches!(instrs[1], Instruction::JumpI { .. }));
    assert_eq!(instrs[2], Instruction::PushC1 { val: [42] });
    assert_eq!(instrs[3], Instruction::Halt {});
}

#[test]
fn jump_with_raw_integer_offset() {
    let instrs = asm("JUMP 3");
    assert_eq!(instrs[0], Instruction::Jump { offset: 3 });
}

#[test]
fn jump_with_negative_integer_offset() {
    let instrs = asm("JUMP -4");
    assert_eq!(instrs[0], Instruction::Jump { offset: -4 });
}

// ---------------------------------------------------------------------------
// QUBO / quantum-specific instructions
// ---------------------------------------------------------------------------

#[test]
fn bqmx_allocate() {
    let instrs = asm("PUSH 4\nBQMX r0");
    assert_eq!(instrs[0], Instruction::PushC1 { val: [4] });
    assert_eq!(instrs[1], Instruction::Bqmx { reg: Register(0) });
}

#[test]
fn setquad_instruction() {
    let instrs = asm("PUSH -1\nPUSH 2\nPUSH 2\nSETQUAD r0");
    assert_eq!(instrs[0], Instruction::PushC1 { val: [0xFF] });
    assert_eq!(instrs[3], Instruction::SetQuad { reg: Register(0) });
}

#[test]
fn vec_operations() {
    let instrs = asm("VECI r2\nPUSH 7\nVECPUSH r2\nPUSH 0\nVECGET r2");
    assert_eq!(instrs[0], Instruction::VecI { reg: Register(2) });
    assert_eq!(instrs[2], Instruction::VecPush { reg: Register(2) });
    assert_eq!(instrs[4], Instruction::VecGet { reg: Register(2) });
}

// ---------------------------------------------------------------------------
// Whitespace and comment tolerance
// ---------------------------------------------------------------------------

#[test]
fn inline_comments_ignored() {
    let instrs = asm("PUSH 1 ; push one\nHALT ; stop");
    assert_eq!(instrs.len(), 2);
    assert_eq!(instrs[0], Instruction::PushC1 { val: [1] });
}

#[test]
fn blank_lines_and_indentation_ignored() {
    let src = "\n\n    PUSH 1\n\n    HALT\n\n";
    let instrs = asm(src);
    assert_eq!(instrs.len(), 2);
}

#[test]
fn hex_and_decimal_literals() {
    let instrs = asm("PUSH 0xFF\nPUSH 255");
    assert_eq!(instrs[0], Instruction::PushC2 { val: [0x00, 0xFF] });
    assert_eq!(instrs[1], Instruction::PushC2 { val: [0x00, 0xFF] });
}

// ---------------------------------------------------------------------------
// Case-insensitive mnemonics
// ---------------------------------------------------------------------------

#[test]
fn lowercase_mnemonics_assemble() {
    let instrs = asm("push 5\npush 3\nadd\nhalt");
    assert_eq!(instrs[0], Instruction::PushC1 { val: [5] });
    assert_eq!(instrs[1], Instruction::PushC1 { val: [3] });
    assert_eq!(instrs[2], Instruction::Add {});
    assert_eq!(instrs[3], Instruction::Halt {});
}

#[test]
fn mixed_case_mnemonics_assemble() {
    let instrs = asm("pUsH 7\nHaLt");
    assert_eq!(instrs[0], Instruction::PushC1 { val: [7] });
    assert_eq!(instrs[1], Instruction::Halt {});
}

#[test]
fn mixed_case_produces_same_bytecode_as_uppercase() {
    let upper = assemble_source("PUSH 1\nADD\nHALT").unwrap();
    let lower = assemble_source("push 1\nadd\nhalt").unwrap();
    let mixed = assemble_source("Push 1\nAdd\nHalt").unwrap();
    assert_eq!(upper.code(), lower.code());
    assert_eq!(upper.code(), mixed.code());
}

#[test]
fn case_insensitive_jump_mnemonics() {
    let instrs = asm("jump done\nnop\ndone:\nhalt");
    assert_eq!(instrs[0], Instruction::Jump { offset: 4 });
    assert_eq!(instrs[1], Instruction::Nop {});
    assert_eq!(instrs[2], Instruction::Halt {});
}

// ---------------------------------------------------------------------------
// PUSH auto-promotion
// ---------------------------------------------------------------------------

#[test]
fn push_large_value_encodes_inline() {
    // 100000 = 0x0001_86A0 encodes inline as PUSHC_3 -- no constant pool needed.
    let program = assemble_source("PUSH 100000\nHALT").expect("assemble failed");
    let instrs = decode_all(program.code());
    assert_eq!(instrs[0], Instruction::PushC3 { val: [0x01, 0x86, 0xA0] });
    assert_eq!(instrs[1], Instruction::Halt {});
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn unknown_mnemonic_is_error() {
    assert!(assemble_source("INVALID_OPCODE").is_err());
}

#[test]
fn undefined_label_is_error() {
    assert!(assemble_source("JUMP nonexistent").is_err());
}

#[test]
fn duplicate_label_is_error() {
    assert!(assemble_source("top:\nNOP\ntop:\nHALT").is_err());
}

#[test]
fn wrong_operand_count_is_error() {
    assert!(assemble_source("ADD r0").is_err());
}

#[test]
fn register_where_integer_expected_is_error() {
    assert!(assemble_source("PUSH r0").is_err());
}

#[test]
fn integer_where_register_expected_is_error() {
    assert!(assemble_source("LOAD 42").is_err());
}

#[test]
fn register_out_of_range_parse_error() {
    assert!(assemble_source("LOAD r256").is_err());
}

#[test]
fn integer_offset_overflow_is_error() {
    // 40000 > i16::MAX (32767)
    assert!(assemble_source("JUMP 40000").is_err());
}
