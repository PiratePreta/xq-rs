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

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "CLI binary: stdout and stderr output are intentional"
)]

//! `xquad` -- unified CLI driver for the XQVM toolchain.

mod asm;
mod dism;
mod run;

use clap::{Parser, Subcommand};

/// `XQuad` toolchain driver.
#[derive(Debug, Parser)]
#[command(name = "xquad", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Assemble XQVM source into bytecode.
    Asm(asm::Args),
    /// Disassemble XQVM bytecode into a human-readable listing.
    Dism(dism::Args),
    /// Run XQVM bytecode or assembly.
    Run(run::Args),
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Asm(args) => asm::exec(args),
        Command::Dism(args) => dism::exec(&args),
        Command::Run(args) => run::exec(args),
    }
}
