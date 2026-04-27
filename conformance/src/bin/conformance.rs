// Copyright (C) 2026 Postquant Labs Incorporated
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Manual runner for the XQVM conformance harness.
//!
//! `cargo test -p xquad-conformance` is the canonical CI entry point.
//! This binary exists for local iteration — it produces human-readable
//! output when authoring a new vector or triaging a specific failure.
//!
//! Example:
//!
//! ```sh
//! cargo run -p xquad-conformance -- --impl both
//! cargo run -p xquad-conformance -- --filter arithmetic --impl rust
//! cargo run -p xquad-conformance -- --filter energy/bqmx_trivial --impl python
//! ```

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::expect_used,
    reason = "this is a CLI binary whose whole purpose is user-facing output"
)]

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use xquad_conformance::{Impl, check, load_vector, run_python, run_rust};

fn main() -> ExitCode {
    let args = Args::parse_env();

    let vectors = discover(args.filter.as_deref());
    if vectors.is_empty() {
        eprintln!("no vectors matched filter {:?}", args.filter);
        return ExitCode::from(2);
    }

    let impls: &[Impl] = match args.which {
        Which::Rust => &[Impl::Rust],
        Which::Python => &[Impl::Python],
        Which::Both => &[Impl::Rust, Impl::Python],
    };

    let mut failures = 0usize;
    let mut total = 0usize;
    for (category, name) in &vectors {
        let vector = match load_vector(category, name) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[load] {category}/{name}: {e}");
                failures += 1;
                continue;
            }
        };
        for &imp in impls {
            total += 1;
            let outcome = match imp {
                Impl::Rust => run_rust(&vector),
                Impl::Python => run_python(&vector),
            };
            match outcome.and_then(|o| check(&o, &vector.expected)) {
                Ok(()) => println!("PASS [{imp:?}] {category}/{name}"),
                Err(e) => {
                    eprintln!("FAIL [{imp:?}] {category}/{name}\n{e}");
                    failures += 1;
                }
            }
        }
    }

    println!("{}/{} checks passed", total - failures, total);
    if failures > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}

#[derive(Debug)]
struct Args {
    which: Which,
    filter: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum Which {
    Rust,
    Python,
    Both,
}

impl Args {
    fn parse_env() -> Self {
        let mut which = Which::Both;
        let mut filter: Option<String> = None;
        let argv: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < argv.len() {
            match argv.get(i).map_or("", String::as_str) {
                "--impl" => {
                    i += 1;
                    which = match argv.get(i).map(String::as_str) {
                        Some("rust") => Which::Rust,
                        Some("python") => Which::Python,
                        Some("both") => Which::Both,
                        other => {
                            eprintln!("--impl expects rust|python|both, got {other:?}");
                            std::process::exit(2);
                        }
                    };
                }
                "--filter" => {
                    i += 1;
                    filter = argv.get(i).cloned();
                }
                "--help" | "-h" => {
                    println!(
                        "usage: conformance [--impl rust|python|both] [--filter <category>[/<name>]]"
                    );
                    std::process::exit(0);
                }
                unknown => {
                    eprintln!("unknown argument: {unknown}");
                    std::process::exit(2);
                }
            }
            i += 1;
        }
        Self { which, filter }
    }
}

fn discover(filter: Option<&str>) -> Vec<(String, String)> {
    let root: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vectors");
    let mut out = Vec::new();
    if !root.exists() {
        return out;
    }
    let categories = fs::read_dir(&root).expect("read vectors/");
    for cat_entry in categories.flatten() {
        if !cat_entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let category = cat_entry.file_name().to_string_lossy().into_owned();
        if let Some(f) = filter
            && !prefix_matches(&category, f)
        {
            continue;
        }
        for vec_entry in fs::read_dir(cat_entry.path())
            .expect("read category")
            .flatten()
        {
            if !vec_entry.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let name = vec_entry.file_name().to_string_lossy().into_owned();
            if let Some(f) = filter {
                let full = format!("{category}/{name}");
                if !(prefix_matches(&category, f) && (f == category || prefix_matches(&full, f))) {
                    continue;
                }
            }
            out.push((category.clone(), name));
        }
    }
    out.sort();
    out
}

fn prefix_matches(candidate: &str, filter: &str) -> bool {
    candidate == filter
        || candidate.starts_with(&format!("{filter}/"))
        || filter.starts_with(candidate)
}
