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

//! Showcase example for the XQVM assembler.
//!
//! Assembles two small programs from inline assembly source and verifies the
//! decoded instruction streams.  Run with:
//!
//! ```text
//! cargo run --example showcase -p aglais-xqvm-asm
//! ```

use aglais_xqvm_asm::assemble_source;
use aglais_xqvm_bytecode::stream::InstructionStream;
use aglais_xqvm_bytecode::types::Instruction;
use miette::{IntoDiagnostic, WrapErr};

fn main() -> miette::Result<()> {
    // -----------------------------------------------------------------------
    // Program 1: sum of 1 + 2 + ... + 5
    //
    // Uses a counted-down loop via PUSH/JUMPI.  The register r0 accumulates
    // the running total; r1 holds the current counter value.
    // -----------------------------------------------------------------------
    let sum_src = "
        ; Compute 1 + 2 + 3 + 4 + 5 in r0.
        ;
        ; r0 = accumulator (starts at 0)
        ; r1 = counter     (counts 5 down to 1)

        PUSH 0
        STOW r0          ; r0 = 0

        PUSH 5
        STOW r1          ; r1 = 5

    loop:
        LOAD r1          ; push counter
        LOAD r0          ; push accumulator
        ADD              ; push counter + accumulator
        STOW r0          ; r0 = acc + counter

        LOAD r1          ; push counter
        PUSH -1
        ADD              ; push counter - 1
        STOW r1          ; r1 = counter - 1

        LOAD r1          ; push counter for the conditional
        JUMPI loop       ; if counter != 0, repeat

        HALT
    ";

    let bytecode = assemble_source(sum_src).wrap_err("failed to assemble sum program")?;
    println!("sum program: {} bytes", bytecode.len());
    print_disasm(&bytecode)?;

    // -----------------------------------------------------------------------
    // Program 2: allocate a 4-variable binary QUBO model, set a diagonal
    // coefficient, then halt.
    // -----------------------------------------------------------------------
    let qubo_src = "
        ; Build a 4-variable binary QUBO model in r0, set Q[2,2] = -1.

        PUSH 4
        BQMX r0          ; r0 = alloc QUBO(4)

        PUSH -1          ; value
        PUSH 2           ; i
        PUSH 2           ; j
        SETQUAD r0       ; model.quadratic[2,2] = -1

        HALT
    ";

    let qubo_bytecode = assemble_source(qubo_src).wrap_err("failed to assemble QUBO program")?;
    println!("QUBO program: {} bytes", qubo_bytecode.len());
    print_disasm(&qubo_bytecode)?;

    // -----------------------------------------------------------------------
    // Verify sum program instruction sequence
    // -----------------------------------------------------------------------
    let instrs: Vec<Instruction> = InstructionStream::new(&bytecode)
        .map(|r| r.map(|(_, _, instr)| instr))
        .collect::<Result<_, _>>()
        .into_diagnostic()
        .wrap_err("failed to decode sum program bytecode")?;

    // First instruction must be PUSH 0
    assert_eq!(instrs[0], Instruction::Push { imm: 0 });
    // Last instruction must be HALT
    let last = instrs
        .last()
        .ok_or_else(|| miette::miette!("sum program produced an empty instruction stream"))?;
    assert_eq!(*last, Instruction::Halt {});

    println!("\nAll checks passed.");
    Ok(())
}

fn print_disasm(buf: &[u8]) -> miette::Result<()> {
    for result in InstructionStream::new(buf) {
        let (offset, _len, instr) = result.into_diagnostic()?;
        println!("  {:04X}  {} {:?}", offset, instr.mnemonic(), instr);
    }
    Ok(())
}
