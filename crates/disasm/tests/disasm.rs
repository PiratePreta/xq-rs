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

//! Smoke tests for the `disasm` binary.

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use aglais_xqvm_bytecode::builder::InstructionBuilder;
use aglais_xqvm_bytecode::codec;
use aglais_xqvm_bytecode::types::{Instruction, Register};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn xqdism() -> Command {
    Command::new(env!("CARGO_BIN_EXE_xqdism"))
}

/// Run `disasm` with `input` piped to stdin; return the captured output.
fn run_stdin(input: &[u8]) -> Output {
    let mut child = xqdism()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn disasm");

    child
        .stdin
        .take()
        .expect("stdin not captured")
        .write_all(input)
        .expect("failed to write stdin");

    child.wait_with_output().expect("failed to wait on disasm")
}

/// Run `xqdism` with `path` as the file argument; return the captured output.
fn run_file(path: &Path) -> Output {
    xqdism()
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("failed to run disasm")
}

/// Encode a slice of instructions into a byte buffer.
fn assemble(instrs: &[Instruction]) -> Vec<u8> {
    instrs.iter().flat_map(codec::encode).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn empty_stdin_produces_empty_output() {
    let out = run_stdin(&[]);
    assert!(out.status.success());
    assert_eq!(out.stdout, b"");
}

#[test]
fn push_halt_contains_mnemonic_and_immediate() {
    let bytes = assemble(&[Instruction::Push { imm: 42 }, Instruction::Halt {}]);
    let out = run_stdin(&bytes);
    assert!(out.status.success(), "exit: {}", out.status);

    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("PUSH"), "missing PUSH in:\n{text}");
    assert!(text.contains("42"), "missing immediate 42 in:\n{text}");
    assert!(text.contains("HALT"), "missing HALT in:\n{text}");
}

#[test]
fn byte_offsets_appear_in_output() {
    // PUSH(0) is 2 bytes so HALT starts at 0x0002.
    let bytes = assemble(&[Instruction::Push { imm: 0 }, Instruction::Halt {}]);
    let out = run_stdin(&bytes);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("0x0000"), "missing 0x0000 in:\n{text}");
    assert!(text.contains("0x0002"), "missing 0x0002 in:\n{text}");
}

#[test]
fn jump_target_gets_label() {
    let mut b = InstructionBuilder::new();
    let top = b.label();
    let _ = b.place(top).nop().jump(top);
    let bytes = b.build().unwrap();

    let out = run_stdin(&bytes);
    assert!(out.status.success(), "exit: {}", out.status);

    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("L0:"), "missing label L0 in:\n{text}");
    assert!(text.contains("JUMP"), "missing JUMP in:\n{text}");
    // Jump operand should resolve to the label name, not a raw offset.
    assert!(
        text.contains("L0"),
        "jump operand should show L0 in:\n{text}"
    );
}

#[test]
fn conditional_jump_shows_label() {
    let mut b = InstructionBuilder::new();
    let done = b.label();
    let _ = b.push(0).jump_if(done).push(1).place(done).halt();
    let bytes = b.build().unwrap();

    let out = run_stdin(&bytes);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("JUMPI"), "missing JUMPI in:\n{text}");
    assert!(text.contains("L0:"), "missing label in:\n{text}");
}

#[test]
fn register_operand_displayed_correctly() {
    let bytes = assemble(&[Instruction::Load { reg: Register(5) }]);
    let out = run_stdin(&bytes);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("LOAD"), "missing LOAD in:\n{text}");
    assert!(text.contains("r5"), "missing r5 in:\n{text}");
}

#[test]
fn energy_shows_both_registers() {
    let bytes = assemble(&[Instruction::Energy {
        model: Register(1),
        sample: Register(2),
    }]);
    let out = run_stdin(&bytes);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("ENERGY"), "missing ENERGY in:\n{text}");
    assert!(text.contains("r1"), "missing r1 in:\n{text}");
    assert!(text.contains("r2"), "missing r2 in:\n{text}");
}

#[test]
fn unknown_byte_renders_as_dot_byte() {
    let out = run_stdin(&[0xFE]);
    assert!(out.status.success(), "exit: {}", out.status);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains(".byte"), "missing .byte in:\n{text}");
    assert!(text.contains("0xFE"), "missing 0xFE in:\n{text}");
}

#[test]
fn file_argument_works() {
    let bytes = assemble(&[Instruction::Push { imm: 7 }, Instruction::Halt {}]);
    let path = std::env::temp_dir().join("disasm_smoke_test.bin");
    fs::write(&path, &bytes).expect("failed to write temp file");

    let out = run_file(&path);
    let _ = fs::remove_file(&path);

    assert!(out.status.success(), "exit: {}", out.status);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("PUSH"), "missing PUSH in:\n{text}");
    assert!(text.contains("7"), "missing immediate 7 in:\n{text}");
}

#[test]
fn missing_file_exits_nonzero_with_error_message() {
    let out = run_file(Path::new("/nonexistent/path/to/file.bin"));
    assert!(
        !out.status.success(),
        "expected non-zero exit for missing file"
    );
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        !stderr.is_empty(),
        "expected an error message on stderr for missing file"
    );
}
