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

//! `xqasm` -- XQVM assembler CLI.
//!
//! Reads an XQVM assembly source file and writes the binary bytecode to an
//! output file (default: `<input>.xqb`) or stdout when `--stdout` is given.

use std::path::PathBuf;

use clap::Parser;
use miette::{IntoDiagnostic, WrapErr};

/// Assemble an XQVM assembly source file into binary bytecode.
#[derive(Parser, Debug)]
#[command(name = "xqasm", version, about)]
struct Args {
    /// Input assembly source file (`.asm`).
    input: PathBuf,

    /// Output file.  Defaults to `<input>.xqb` when omitted.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Write bytecode to stdout instead of a file.
    #[arg(long, conflicts_with = "output")]
    stdout: bool,
}

fn main() -> miette::Result<()> {
    let args = Args::parse();

    let source = std::fs::read_to_string(&args.input)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read '{}'", args.input.display()))?;

    let name = args.input.display().to_string();
    let lines = aglais_xqvm_asm::parser::parse(&source, &name)?;
    let bytecode = aglais_xqvm_asm::assembler::assemble(&lines, &source, &name)?;

    if args.stdout {
        use std::io::Write as _;
        std::io::stdout()
            .write_all(&bytecode)
            .into_diagnostic()
            .wrap_err("failed to write bytecode to stdout")?;
    } else {
        let out_path = args.output.unwrap_or_else(|| {
            let mut p = args.input.clone();
            p.set_extension("xqb");
            p
        });
        std::fs::write(&out_path, &bytecode)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to write '{}'", out_path.display()))?;
        eprintln!(
            "assembled {} instructions ({} bytes) -> {}",
            instruction_count(&bytecode),
            bytecode.len(),
            out_path.display(),
        );
    }

    Ok(())
}

/// Estimate the number of instructions by decoding the buffer.
fn instruction_count(buf: &[u8]) -> usize {
    aglais_xqvm_bytecode::stream::InstructionStream::new(buf)
        .filter(|r| r.is_ok())
        .count()
}
