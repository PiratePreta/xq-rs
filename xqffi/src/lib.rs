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

//! `PyO3` bindings for the `XQuad` Rust runtime.
//!
//! Exposes two Python submodules:
//!
//! - `xqffi.asm` — bindings around [`xqasm`]: `parse_xqasm`,
//!   `assemble_source`, `disassemble`, `instruction_count`.
//! - `xqffi.vm` — bindings around [`xqvm::Vm`]: a low-level `Vm` class
//!   with the `set_calldata` / `set_output_slots` / `run` / `outputs` /
//!   `stack` surface that the conformance harness drives, plus thin
//!   `XqmxModel` / `XqmxSample` wrappers with getters and setters.
//!
//! User-facing convenience (`Program`, `Session`, `RunResult`, model/sample
//! dict conversion) lives in the `xquad` pure-Python umbrella package.
//!
//! The crate ships as a Python extension (`cdylib`) built via `maturin`;
//! the `rlib` output is also enabled so Rust tests and in-workspace
//! consumers can import it without going through the Python layer.

use pyo3::prelude::*;

mod asm;
mod vm;

/// Module entry point — registered as `xqffi` by maturin (via the
/// `module-name` field in `pyproject.toml`, which matches the `[lib] name`
/// in `Cargo.toml`).
///
/// Submodules (`xqffi.asm`, `xqffi.vm`) are registered both as
/// attributes of the parent module *and* in `sys.modules` so
/// `from xqffi.asm import parse_xqasm` resolves. `add_submodule`
/// alone only sets the attribute; pyo3's module tree doesn't
/// automatically populate `sys.modules` the way `__init__.py`-based
/// packages do.
#[pymodule]
fn xqffi(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let sys_modules = py.import("sys")?.getattr("modules")?;

    let asm_module = PyModule::new(py, "xqffi.asm")?;
    asm::register(py, &asm_module)?;
    m.add_submodule(&asm_module)?;
    sys_modules.set_item("xqffi.asm", &asm_module)?;

    let vm_module = PyModule::new(py, "xqffi.vm")?;
    vm::register(py, &vm_module)?;
    m.add_submodule(&vm_module)?;
    sys_modules.set_item("xqffi.vm", &vm_module)?;

    Ok(())
}
