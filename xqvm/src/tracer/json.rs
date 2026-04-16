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

use std::fmt::Write as _;
use std::io::Write;

use crate::tracer::{StepState, Tracer};
use crate::value::RegVal;

/// Writes one JSON object per line (JSONL format).
///
/// Each call to [`on_step`](Tracer::on_step) emits a single line containing
/// a JSON object with fields: `step`, `pos`, `instruction`, `stack`,
/// `read_regs`, and `written_regs`.
///
/// No serde dependency is used; serialization is hand-written.
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

/// Escape a string for JSON output.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// Format a register value as a JSON object (without surrounding braces
/// of the containing map).
fn regval_json(val: &RegVal) -> String {
    match val {
        RegVal::Unset => "{\"type\":\"unset\"}".to_string(),
        RegVal::Int(v) => {
            format!("{{\"type\":\"int\",\"value\":{v}}}")
        }
        RegVal::VecInt(v) => {
            format!("{{\"type\":\"vec<int>\",\"len\":{}}}", v.len())
        }
        RegVal::VecXqmx(v) => {
            format!("{{\"type\":\"vec<model>\",\"len\":{}}}", v.len())
        }
        RegVal::Model(m) => {
            format!(
                "{{\"type\":\"model\",\"rows\":{},\"cols\":{}}}",
                m.rows, m.cols
            )
        }
        RegVal::Sample(s) => {
            format!("{{\"type\":\"sample\",\"len\":{}}}", s.values.len())
        }
    }
}

/// Format a register list as a JSON object: `{"0":{...}, "2":{...}}`.
fn regs_json(regs: &[(u8, RegVal)]) -> String {
    let mut out = String::from("{");
    for (i, (idx, val)) in regs.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "\"{}\":{}", idx, regval_json(val));
    }
    out.push('}');
    out
}

/// Format a stack as a JSON array.
fn stack_json(stack: &[i64]) -> String {
    let mut out = String::from("[");
    for (i, v) in stack.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{v}");
    }
    out.push(']');
    out
}

impl<W: Write> Tracer for JsonTracer<W> {
    type Error = std::io::Error;

    fn on_step(&mut self, state: &StepState<'_>) -> Result<(), Self::Error> {
        let instr = json_escape(&format!("{}", state.instruction));
        writeln!(
            self.out,
            "{{\"step\":{},\"pos\":{},\"instruction\":\"{}\",\"stack\":{},\"read_regs\":{},\"written_regs\":{}}}",
            state.step,
            state.pos,
            instr,
            stack_json(state.stack),
            regs_json(state.read_regs),
            regs_json(state.written_regs),
        )?;
        Ok(())
    }
}
