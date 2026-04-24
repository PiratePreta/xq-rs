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

//! Register value types for the XQVM interpreter.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

pub(crate) use crate::model::{XqmxModel, XqmxSample};

/// A value that can be stored in an XQVM register.
///
/// Registers begin in the `Unset` state. `STOW`, `INPUT`, `LVAL`, `LIDX`, and
/// the allocator instructions (`BQMX`, `VECI`, etc.) transition a register to
/// one of the typed variants. `DROP` returns it to `Unset`. A `LOAD` on an
/// `Unset` register is a runtime fault (`Error::UnsetRegister`).
#[derive(Debug, Clone, PartialEq)]
pub enum RegVal {
    /// Register has no value; `LOAD` on this slot faults.
    Unset,
    /// Integer. Exchanges values with the stack via `LOAD`/`STOW`.
    Int(i64),
    /// Integer vector, created by `VECI` or `VEC`.
    VecInt(Vec<i64>),
    /// XQMX model vector, created by `VECX`.
    VecXqmx(Vec<XqmxModel>),
    /// XQMX model (QUBO/Ising/discrete), created by `BQMX`/`SQMX`/`XQMX`.
    Model(XqmxModel),
    /// XQMX sample, created by `BSMX`/`SSMX`/`XSMX`.
    Sample(XqmxSample),
}

impl Default for RegVal {
    /// An uninitialised register. Returning `Unset` (rather than `Int(0)`)
    /// means any `vec![RegVal::default(); n]` buffer starts in the same
    /// state as a freshly-constructed `Vm`, and a `LOAD` from such a
    /// register faults — matching `spec/xqvm/SPEC.md:46`, which treats
    /// unwritten slots as absent, not zero.
    fn default() -> Self {
        Self::Unset
    }
}

impl RegVal {
    /// Return the integer value, or an error string if the wrong type.
    pub(crate) fn as_int(&self) -> Result<i64, &'static str> {
        match self {
            Self::Int(n) => Ok(*n),
            Self::Unset => Err("unset"),
            Self::VecInt(_) => Err("vec<int>"),
            Self::VecXqmx(_) => Err("vec<xqmx>"),
            Self::Model(_) => Err("model"),
            Self::Sample(_) => Err("sample"),
        }
    }

    /// Return a mutable reference to the model, or an error string.
    pub(crate) fn as_model_mut(&mut self) -> Result<&mut XqmxModel, &'static str> {
        match self {
            Self::Model(m) => Ok(m),
            Self::Unset => Err("unset"),
            Self::Int(_) => Err("int"),
            Self::VecInt(_) => Err("vec<int>"),
            Self::VecXqmx(_) => Err("vec<xqmx>"),
            Self::Sample(_) => Err("sample"),
        }
    }

    /// Return a shared reference to the model, or an error string.
    pub(crate) fn as_model(&self) -> Result<&XqmxModel, &'static str> {
        match self {
            Self::Model(m) => Ok(m),
            Self::Unset => Err("unset"),
            Self::Int(_) => Err("int"),
            Self::VecInt(_) => Err("vec<int>"),
            Self::VecXqmx(_) => Err("vec<xqmx>"),
            Self::Sample(_) => Err("sample"),
        }
    }

    /// Return a mutable reference to the int vec, or an error string.
    pub(crate) fn as_vec_int_mut(&mut self) -> Result<&mut Vec<i64>, &'static str> {
        match self {
            Self::VecInt(v) => Ok(v),
            Self::Unset => Err("unset"),
            Self::Int(_) => Err("int"),
            Self::VecXqmx(_) => Err("vec<xqmx>"),
            Self::Model(_) => Err("model"),
            Self::Sample(_) => Err("sample"),
        }
    }

    /// Return a shared reference to the int vec, or an error string.
    pub(crate) fn as_vec_int(&self) -> Result<&Vec<i64>, &'static str> {
        match self {
            Self::VecInt(v) => Ok(v),
            Self::Unset => Err("unset"),
            Self::Int(_) => Err("int"),
            Self::VecXqmx(_) => Err("vec<xqmx>"),
            Self::Model(_) => Err("model"),
            Self::Sample(_) => Err("sample"),
        }
    }

    /// Type name for error messages.
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            Self::Unset => "unset",
            Self::Int(_) => "int",
            Self::VecInt(_) => "vec<int>",
            Self::VecXqmx(_) => "vec<xqmx>",
            Self::Model(_) => "model",
            Self::Sample(_) => "sample",
        }
    }

    /// Return an immutable XQMX grid view if this register holds a
    /// model or a sample. Used by grid opcodes (`ROWSUM`, `COLSUM`,
    /// `ROWFIND`, `COLFIND`) per `spec/xqvm/SPEC.md` §303, which accept
    /// either XQMX mode.
    pub(crate) fn as_xqmx_grid(&self) -> Result<XqmxGridRef<'_>, &'static str> {
        match self {
            Self::Model(m) => Ok(XqmxGridRef::Model(m)),
            Self::Sample(s) => Ok(XqmxGridRef::Sample(s)),
            Self::Unset => Err("unset"),
            Self::Int(_) => Err("int"),
            Self::VecInt(_) => Err("vec<int>"),
            Self::VecXqmx(_) => Err("vec<xqmx>"),
        }
    }

    /// Return a mutable XQMX grid view for `RESIZE` — the only
    /// grid opcode that mutates the register.
    pub(crate) fn as_xqmx_grid_mut(&mut self) -> Result<XqmxGridRefMut<'_>, &'static str> {
        match self {
            Self::Model(m) => Ok(XqmxGridRefMut::Model(m)),
            Self::Sample(s) => Ok(XqmxGridRefMut::Sample(s)),
            Self::Unset => Err("unset"),
            Self::Int(_) => Err("int"),
            Self::VecInt(_) => Err("vec<int>"),
            Self::VecXqmx(_) => Err("vec<xqmx>"),
        }
    }
}

/// Immutable view of the grid-addressed surface of an XQMX register.
///
/// Shared by [`RegVal::Model`] and [`RegVal::Sample`]. Models store
/// `linear` sparsely (`BTreeMap<usize, i64>`, 0 for absent entries);
/// samples store `values` densely (`Vec<i64>`, 0 for out-of-bounds).
pub(crate) enum XqmxGridRef<'a> {
    Model(&'a XqmxModel),
    Sample(&'a XqmxSample),
}

impl XqmxGridRef<'_> {
    pub(crate) fn rows(&self) -> usize {
        match self {
            Self::Model(m) => m.rows,
            Self::Sample(s) => s.rows,
        }
    }

    pub(crate) fn cols(&self) -> usize {
        match self {
            Self::Model(m) => m.cols,
            Self::Sample(s) => s.cols,
        }
    }

    /// Size of the linear surface — number of addressable variables.
    /// For models this is `model.size`; for samples it is
    /// `sample.values.len()`.
    pub(crate) fn size(&self) -> usize {
        match self {
            Self::Model(m) => m.size,
            Self::Sample(s) => s.values.len(),
        }
    }

    /// Read `linear[idx]` — sparse lookup for models (0 if absent),
    /// dense lookup for samples (0 if out-of-bounds).
    pub(crate) fn linear(&self, idx: usize) -> i64 {
        match self {
            Self::Model(m) => m.get_linear(idx),
            Self::Sample(s) => s.values.get(idx).copied().unwrap_or(0),
        }
    }
}

/// Mutable view for `RESIZE`.
pub(crate) enum XqmxGridRefMut<'a> {
    Model(&'a mut XqmxModel),
    Sample(&'a mut XqmxSample),
}

impl XqmxGridRefMut<'_> {
    pub(crate) fn set_grid(&mut self, rows: usize, cols: usize) {
        match self {
            Self::Model(m) => {
                m.rows = rows;
                m.cols = cols;
            }
            Self::Sample(s) => {
                s.rows = rows;
                s.cols = cols;
            }
        }
    }

    pub(crate) fn size(&self) -> usize {
        match self {
            Self::Model(m) => m.size,
            Self::Sample(s) => s.values.len(),
        }
    }

    /// Write `linear[idx] = val`. Indexing is sparse for models
    /// (zero values drop the entry) and dense for samples.
    pub(crate) fn linear_set(&mut self, idx: usize, val: i64) {
        match self {
            Self::Model(m) => m.set_linear(idx, val),
            Self::Sample(s) => {
                if let Some(slot) = s.values.get_mut(idx) {
                    *slot = val;
                }
            }
        }
    }

    /// `linear[idx] += delta`. For models this uses the sparse-aware
    /// accumulator; for samples it's a direct in-place add.
    pub(crate) fn linear_add(&mut self, idx: usize, delta: i64) {
        match self {
            Self::Model(m) => m.add_linear(idx, delta),
            Self::Sample(s) => {
                if let Some(slot) = s.values.get_mut(idx) {
                    *slot += delta;
                }
            }
        }
    }
}
