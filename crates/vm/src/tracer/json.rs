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

//! JSONL tracer.

use std::io::Write;

use crate::tracer::{StepState, Tracer};

/// Writes one JSON object per line (JSONL format).
#[derive(Debug)]
pub struct JsonTracer<W: Write> {
    out: W,
}

impl<W: Write> JsonTracer<W> {
    /// Create a new JSON tracer writing to `out`.
    pub fn new(out: W) -> Self {
        Self { out }
    }
}

impl<W: Write> Tracer for JsonTracer<W> {
    type Error = std::io::Error;

    fn on_step(&mut self, _state: &StepState<'_>) -> Result<(), Self::Error> {
        todo!("implemented in Task 7")
    }
}
