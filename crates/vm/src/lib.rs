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

//! XQVM bytecode interpreter.
//!
//! This crate provides a stack-based interpreter for the Aglais X-Quadratic
//! Virtual Machine bytecode. It accepts either a pre-assembled binary buffer
//! or, through the `assemble_source` re-export, a raw assembly text string.
//!
//! | Item | Description |
//! |---|---|
//! | [`Vm`] | The interpreter -- stack, registers, loop stack |
//! | [`Error`] | Runtime fault variants |
//! | [`XqmxModel`] | QUBO/Ising/discrete optimization model |
//! | [`XqmxSample`] | Candidate solution for an XQMX model |
//! | [`RegVal`] | Register value type |
//!
//! # Quick start
//!
//! ```rust
//! use aglais_xqvm_vm::Vm;
//! use aglais_xqvm_bytecode::InstructionBuilder;
//!
//! let mut b = InstructionBuilder::new();
//! b.push(10).push(32).add().halt();
//! let program = b.build().unwrap();
//!
//! let mut vm = Vm::new();
//! vm.run(&program).unwrap();
//! assert_eq!(vm.stack(), &[42]);
//! ```

// No standard library when the `std` feature is disabled (e.g. WASM targets).
// The `alloc` crate provides heap types (`Vec`, `BTreeMap`, ...).
#![cfg_attr(not(feature = "std"), no_std)]
// Private modules have doc tests that are only visible to maintainers.
#![allow(rustdoc::private_doc_tests)]

#[cfg(not(feature = "std"))]
extern crate alloc;

mod error;
mod model;
pub mod tracer;
mod value;
mod vm;

// ---------------------------------------------------------------------------
// Public API re-exports
// ---------------------------------------------------------------------------

pub use error::Error;
#[cfg(feature = "std")]
pub use error::RuntimeDiagnostic;
pub use model::{Domain, XqmxModel, XqmxSample};
#[cfg(feature = "std")]
pub use tracer::{JsonTracer, TextTracer};
pub use tracer::{NoopTracer, StepState, Tracer};
pub use value::RegVal;
pub use vm::Vm;
