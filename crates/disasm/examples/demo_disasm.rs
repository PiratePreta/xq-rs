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

//! Disassembler demo.
//!
//! Assembles a small XQVM program that counts down from 5 to 0, allocates a
//! QUBO model and binary sample, computes Hamiltonian energy, then halts.
//! Two invalid bytes are injected before the `ENERGY` instruction to
//! demonstrate `.byte` fallback rendering in the disassembly output.
//!
//! # Run
//!
//! ```text
//! cargo run --example demo_disasm -p aglais-xqvm-disasm
//! ```

use std::io;

use aglais_xqvm_bytecode::builder::InstructionBuilder;
use aglais_xqvm_bytecode::types::Register;
use aglais_xqvm_disasm::display::Disassembly;

fn main() {
    let mut b = InstructionBuilder::new();
    let loop_top = b.label();
    let done = b.label();

    // Count down from 5 to 0.
    let _ = b
        .push(5)
        .place(loop_top)
        .dupl()
        .jump_if(done) // exit when counter reaches 0
        .push(-1)
        .add()
        .jump(loop_top)
        .place(done)
        .pop(); // discard counter

    // Allocate a 4-variable QUBO model into r0 and a binary sample into r1,
    // then compute the Hamiltonian energy of the sample against the model.
    let _ = b
        .push(4)
        .bqmx(Register(0))
        .push(4)
        .bsmx(Register(1))
        .energy(Register(0), Register(1))
        .halt();

    let mut buf = b.build().unwrap().code().to_vec();

    // Inject two invalid bytes immediately before the ENERGY instruction to
    // demonstrate that the disassembler renders them as `.byte 0xXX` and
    // continues decoding subsequent instructions correctly.
    let inject_at = buf.len() - 4; // ENERGY (3 bytes) + HALT (1 byte) = 4 from end
    buf.insert(inject_at, 0xAB);
    buf.insert(inject_at, 0xDE);

    Disassembly::new(&buf).write_to(&mut io::stdout()).unwrap();
}
