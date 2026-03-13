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

//! `disasm` -- XQVM bytecode disassembler.
//!
//! Reads raw XQVM bytecode from a file or stdin and prints a human-readable
//! listing to stdout. Jump targets are automatically labelled `L0`, `L1`, ...
//! in address order.
//!
//! # Usage
//!
//! ```text
//! disasm [FILE]
//! ```
//!
//! When `FILE` is omitted, bytecode is read from standard input.

use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use clap::Parser;
use miette::{IntoDiagnostic, Result, WrapErr};

use aglais_xqvm_disasm::display::Disassembly;

/// Disassemble XQVM bytecode.
///
/// Reads raw bytecode from FILE (or stdin when FILE is omitted) and prints
/// a human-readable listing to stdout. Jump targets are labelled L0, L1, ...
/// in ascending address order.
#[derive(Debug, Parser)]
#[command(name = "disasm", version, about)]
struct Args {
    /// Bytecode file to disassemble. Reads from stdin when omitted.
    file: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let bytes = match args.file {
        Some(path) => fs::read(&path)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to read '{}'", path.display()))?,
        None => {
            let mut buf = Vec::new();
            let _n = io::stdin()
                .read_to_end(&mut buf)
                .into_diagnostic()
                .wrap_err("failed to read stdin")?;
            buf
        }
    };

    Disassembly::new(&bytes)
        .write_to(&mut io::stdout())
        .into_diagnostic()
        .wrap_err("failed to write disassembly")?;

    Ok(())
}
