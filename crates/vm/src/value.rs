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

use crate::model::{XqmxModel, XqmxSample};

/// A value that can be stored in an XQVM register.
///
/// All registers default to `Int(0)`. The type is determined by the allocation
/// instruction used (`BQMX`, `VECI`, etc.).
#[derive(Debug, Clone, PartialEq)]
pub enum RegVal {
    /// Integer (default). Exchanges values with the stack via `LOAD`/`STOW`.
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
    fn default() -> Self {
        Self::Int(0)
    }
}

impl RegVal {
    /// Return the integer value, or an error string if the wrong type.
    pub(crate) fn as_int(&self) -> Result<i64, &'static str> {
        match self {
            Self::Int(n) => Ok(*n),
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
            Self::Int(_) => Err("int"),
            Self::VecXqmx(_) => Err("vec<xqmx>"),
            Self::Model(_) => Err("model"),
            Self::Sample(_) => Err("sample"),
        }
    }

    /// Type name for error messages.
    pub(crate) fn type_name(&self) -> &'static str {
        match self {
            Self::Int(_) => "int",
            Self::VecInt(_) => "vec<int>",
            Self::VecXqmx(_) => "vec<xqmx>",
            Self::Model(_) => "model",
            Self::Sample(_) => "sample",
        }
    }
}
