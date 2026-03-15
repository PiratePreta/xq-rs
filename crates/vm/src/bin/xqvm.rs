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

//! `xqvm` -- XQVM bytecode interpreter CLI.
//!
//! Interprets a bytecode program from a binary file (default) or an
//! assembly text file (`--text`). Writes outputs to stdout.
//!
//! # Usage
//!
//! ```text
//! xqvm [OPTIONS] <FILE>
//! ```

use std::fs;
use std::path::PathBuf;

use clap::Parser;
use miette::{IntoDiagnostic, Result, WrapErr};

use aglais_xqvm_asm::assemble_source;
use aglais_xqvm_bytecode::program::Program;
use aglais_xqvm_vm::value::RegVal;
use aglais_xqvm_vm::vm::Vm;

/// Interpret XQVM bytecode.
///
/// Reads a bytecode program from FILE and executes it. Use --text to
/// accept assembly source instead of a binary file.
#[derive(Debug, Parser)]
#[command(name = "xqvm", version, about)]
struct Args {
    /// Treat FILE as assembly text and assemble before interpreting.
    #[arg(long)]
    text: bool,

    /// Comma-separated calldata integers passed to INPUT instructions.
    #[arg(long, value_delimiter = ',')]
    calldata: Vec<i64>,

    /// Number of output slots available for OUTPUT instructions.
    #[arg(long, default_value = "16")]
    outputs: usize,

    /// Maximum number of instructions to execute (0 = unlimited).
    #[arg(long, default_value = "10000000")]
    step_limit: u64,

    /// Bytecode (or assembly) file to interpret.
    file: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let source = fs::read(&args.file)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read '{}'", args.file.display()))?;

    let program: Program = if args.text {
        let text = String::from_utf8(source)
            .into_diagnostic()
            .wrap_err("assembly file is not valid UTF-8")?;
        assemble_source(&text)
            .map_err(|e| miette::miette!("{e}"))
            .wrap_err("assembly failed")?
    } else {
        Program::decode(&source)
            .into_diagnostic()
            .wrap_err("invalid bytecode: failed to decode program")?
    };

    let calldata: Vec<RegVal> = args.calldata.into_iter().map(RegVal::Int).collect();

    let mut vm = Vm::new();
    vm.set_calldata(calldata).set_output_slots(args.outputs);
    if args.step_limit > 0 {
        vm.set_step_limit(args.step_limit);
    }

    vm.run(&program)
        .map_err(|e| e.into_diagnostic(&program, &args.file.to_string_lossy()))?;

    let outputs = vm.outputs();
    let has_outputs = outputs.iter().any(|v| v != &RegVal::default());
    if has_outputs {
        println!("outputs:");
        for (i, v) in outputs.iter().enumerate() {
            println!("  [{i}] = {v:?}");
        }
    }

    let stack = vm.stack();
    if !stack.is_empty() {
        println!("stack (bottom to top):");
        for v in stack {
            println!("  {v}");
        }
    }

    Ok(())
}
