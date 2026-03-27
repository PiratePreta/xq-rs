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

//! Integration tests for JsonTracer output.

use aglais_xqvm_bytecode::{InstructionBuilder, Register};
use aglais_xqvm_vm::{JsonTracer, Vm};

fn is_valid_json(s: &str) -> bool {
    let s = s.trim();
    s.starts_with('{') && s.ends_with('}')
}

#[test]
fn json_tracer_produces_valid_jsonl() {
    let mut b = InstructionBuilder::new();
    let _ = b.push(3).push(4).add().stow(Register(0)).halt();
    let program = b.build().unwrap();

    let mut buf = Vec::new();
    let mut tracer = JsonTracer::new(&mut buf);
    let mut vm = Vm::new();
    vm.run_trace(&mut tracer, &program).unwrap();

    let output = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = output.lines().collect();

    assert_eq!(lines.len(), 5, "expected 5 lines, got:\n{output}");

    for (i, line) in lines.iter().enumerate() {
        assert!(is_valid_json(line), "line {i} is not valid JSON: {line}");
    }

    assert!(lines[0].contains("\"step\":1"));
    assert!(lines[4].contains("\"step\":5"));

    // STOW r0 (step 4) should show written register.
    assert!(lines[3].contains("\"written_regs\""));
}

#[test]
fn json_tracer_each_line_has_required_fields() {
    let mut b = InstructionBuilder::new();
    let _ = b.push(42).halt();
    let program = b.build().unwrap();

    let mut buf = Vec::new();
    let mut tracer = JsonTracer::new(&mut buf);
    let mut vm = Vm::new();
    vm.run_trace(&mut tracer, &program).unwrap();

    let output = String::from_utf8(buf).unwrap();
    for line in output.lines() {
        assert!(line.contains("\"step\""), "missing step: {line}");
        assert!(line.contains("\"pos\""), "missing pos: {line}");
        assert!(line.contains("\"instruction\""), "missing instruction: {line}");
        assert!(line.contains("\"stack\""), "missing stack: {line}");
    }
}
