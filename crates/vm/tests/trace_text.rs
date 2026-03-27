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

//! Integration tests for TextTracer output.

use aglais_xqvm_bytecode::{InstructionBuilder, Register};
use aglais_xqvm_vm::{TextTracer, Vm};

#[test]
fn text_tracer_produces_header_and_rows() {
    let mut b = InstructionBuilder::new();
    let _ = b.push(3).push(4).add().stow(Register(0)).halt();
    let program = b.build().unwrap();

    let mut buf = Vec::new();
    let mut tracer = TextTracer::new(&mut buf);
    let mut vm = Vm::new();
    vm.run_trace(&mut tracer, &program).unwrap();

    let output = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = output.lines().collect();

    // First line is the column header.
    let header = lines.first().expect("should have header");
    assert!(header.contains("step"));
    assert!(header.contains("instruction"));

    // We should have header + 5 data rows.
    assert_eq!(lines.len(), 6, "header + 5 steps, got:\n{output}");

    // Step 4 (STOW r0) should show a written register.
    let stow_row = lines.get(4).expect("should have row 4");
    assert!(
        stow_row.contains("r0=7"),
        "STOW r0 row should show r0=7, got: {stow_row}",
    );
}

#[test]
fn text_tracer_large_stack_is_truncated() {
    let mut b = InstructionBuilder::new();
    for i in 0..12 {
        let _ = b.push(i);
    }
    let _ = b.halt();
    let program = b.build().unwrap();

    let mut buf = Vec::new();
    let mut tracer = TextTracer::new(&mut buf);
    let mut vm = Vm::new();
    vm.run_trace(&mut tracer, &program).unwrap();

    let output = String::from_utf8(buf).unwrap();
    let last_data = output.lines().last().unwrap();
    assert!(
        last_data.contains("..."),
        "12-element stack should be truncated, got: {last_data}"
    );
}
