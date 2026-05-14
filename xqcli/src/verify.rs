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

//! `xquad verify` subcommand -- runs the full bytecode verifier (all phases).

use std::path::PathBuf;

use clap::Parser;
use miette::{IntoDiagnostic, WrapErr};

use xqasm::assemble_source;
use xqvm::Program;

/// Run the default bytecode verifier over a program.
///
/// Reads FILE (binary bytecode by default), runs all verification phases
/// (structural, jump-target, loop-nesting, register type-state, stack depth)
/// and exits with status 0 on success
/// or 1 on the first violation found.
#[derive(Debug, Parser)]
pub(crate) struct Args {
    /// Treat FILE as assembly text and assemble before verifying.
    #[arg(long)]
    text: bool,

    /// Program file to verify (.xqb by default, .xqasm when --text is set).
    file: PathBuf,
}

/// Execute the `verify` subcommand.
pub(crate) fn run(args: &Args) -> miette::Result<()> {
    let source = std::fs::read(&args.file)
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
            .wrap_err("failed to decode program")?
    };

    xqvm::verifier::verify(&program).map_err(|e| miette::miette!("{e}"))?;

    let count = instruction_count(program.code());
    println!(
        "ok: {} ({} instruction{})",
        args.file.display(),
        count,
        if count == 1 { "" } else { "s" },
    );
    Ok(())
}

fn instruction_count(buf: &[u8]) -> usize {
    xqvm::InstructionStream::new(buf)
        .filter_map(Result::ok)
        .count()
}
