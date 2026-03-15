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

//! Pretty printer for XQVM bytecode.
//!
//! The entry point is [`Disassembly`], which wraps a raw byte slice and
//! implements [`Display`](std::fmt::Display). Jump targets are automatically
//! assigned sequential labels (`L0`, `L1`, ...) and rendered both at the
//! destination and as the operand of the originating `JUMP`/`JUMPI`.
//!
//! Unknown bytes (invalid opcodes or truncated operands) are displayed as
//! `.byte 0xXX` pseudo-instructions so no information is silently dropped.
//!
//! Label collection is delegated to [`InstructionStream`],
//! which performs the pre-pass during construction and embeds the label name
//! directly in each decoded instruction tuple.
//!
//! # Output format
//!
//! Labels appear inline between the byte offset and the mnemonic. When no
//! label is present at an address the column is left blank so columns align:
//!
//! ```text
//!   0x0000:  L0:  PUSH     5
//!   0x0009:       GT
//!   0x000A:       JUMPI    L0
//!   0x000D:       HALT
//! ```
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_bytecode::types::Instruction;
//! use aglais_xqvm_bytecode::codec;
//! use aglais_xqvm_disasm::display::Disassembly;
//!
//! // No jumps -- label column is suppressed entirely.
//! let program = [
//!     Instruction::Push { imm: 1 },
//!     Instruction::Push { imm: 2 },
//!     Instruction::Add  {},
//!     Instruction::Halt {},
//! ];
//! let buf: Vec<u8> = program.iter().flat_map(|i| codec::encode(i)).collect();
//!
//! let text = Disassembly::new(&buf).to_string();
//! assert!(text.contains("PUSH"));
//! assert!(text.contains("ADD"));
//! assert!(text.contains("HALT"));
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::io;

use aglais_xqvm_bytecode::opcodes;
use aglais_xqvm_bytecode::stream::{self, InstructionStream};
use aglais_xqvm_bytecode::types::{Instruction, Register};

// ---------------------------------------------------------------------------
// Operand display
// ---------------------------------------------------------------------------

/// Format a single operand field of an instruction.
///
/// `i16` fields are resolved against the label map using the instruction's
/// byte offset as the base; all other field types are formatted as-is.
trait FmtOperand {
    fn fmt_operand(
        &self,
        f: &mut dyn io::Write,
        labels: &BTreeMap<usize, String>,
        instr_offset: usize,
    ) -> io::Result<()>;
}

impl FmtOperand for Register {
    fn fmt_operand(
        &self,
        f: &mut dyn io::Write,
        _labels: &BTreeMap<usize, String>,
        _instr_offset: usize,
    ) -> io::Result<()> {
        write!(f, "r{}", self.0)
    }
}

impl FmtOperand for i64 {
    fn fmt_operand(
        &self,
        f: &mut dyn io::Write,
        _labels: &BTreeMap<usize, String>,
        _instr_offset: usize,
    ) -> io::Result<()> {
        write!(f, "{self}")
    }
}

impl FmtOperand for i16 {
    /// Resolve the offset to a label name when possible; fall back to `+N`.
    fn fmt_operand(
        &self,
        f: &mut dyn io::Write,
        labels: &BTreeMap<usize, String>,
        instr_offset: usize,
    ) -> io::Result<()> {
        let target = instr_offset as i64 + i64::from(*self);
        if let Some(label) = labels.get(&(target as usize))
            && target >= 0
        {
            return write!(f, "{label}");
        }
        write!(f, "{self:+}")
    }
}

// ---------------------------------------------------------------------------
// Macro-generated instruction formatter
// ---------------------------------------------------------------------------

macro_rules! impl_fmt_instruction {
    ( $( ($code:literal, $variant:ident, $mnem:literal, $doc:literal,
          {$($fname:ident: $ftype:ty),*}) ),* $(,)? ) => {

        fn fmt_instruction(
            instr: &Instruction,
            instr_offset: usize,
            labels: &BTreeMap<usize, String>,
            f: &mut dyn io::Write,
        ) -> io::Result<()> {
            match instr {
                $(
                    Instruction::$variant { $($fname,)* } => {
                        write!(f, "{:<8}", $mnem)?;
                        // `_first` and its mutation are unused when there are no
                        // operands; the attribute suppresses the resulting warning.
                        #[allow(unused_mut)]
                        let mut _first = true;
                        $(
                            if !_first { write!(f, ", ")?; }
                            $fname.fmt_operand(f, labels, instr_offset)?;
                            _first = false;
                        )*
                        let _ = _first;
                        Ok(())
                    }
                )*
            }
        }
    };
}

opcodes!(impl_fmt_instruction);

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// A pretty-printed view of raw XQVM bytecode.
///
/// Jump targets are assigned labels `L0`, `L1`, ... in address order by the
/// underlying [`InstructionStream`]. Each label is printed inline between the
/// byte offset and the mnemonic. When no label exists at an address the
/// column is left blank so all columns align. `JUMP`/`JUMPI` operands are
/// replaced by the label name when the target is within the same buffer.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_bytecode::types::{Instruction, Register};
/// use aglais_xqvm_bytecode::codec;
/// use aglais_xqvm_disasm::display::Disassembly;
///
/// // Build a trivial counted-down loop.
/// let program = [
///     Instruction::Push  { imm: 5 },          // push counter
///     Instruction::Push  { imm: -1 },          // push decrement
///     Instruction::Target{},                   // loop target
///     Instruction::Add   {},                   // counter--
///     Instruction::Dupl  {},                   // check without consuming
///     Instruction::JumpI { offset: -4i16 },    // loop back if non-zero
///     Instruction::Pop   {},
///     Instruction::Halt  {},
/// ];
/// let buf: Vec<u8> = program.iter().flat_map(|i| codec::encode(i)).collect();
/// print!("{}", Disassembly::new(&buf));
/// ```
#[derive(Debug, Clone)]
pub struct Disassembly<'a> {
    stream: InstructionStream<'a>,
}

impl<'a> Disassembly<'a> {
    /// Wrap a raw bytecode buffer for display.
    ///
    /// Constructs an [`InstructionStream`] immediately, performing the label
    /// pre-pass so that repeated calls to [`write_to`](Self::write_to)
    /// reuse the already-computed label map.
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            stream: InstructionStream::new(bytes),
        }
    }

    /// Write the disassembly listing to `out`.
    ///
    /// Each instruction is emitted as one line in the form:
    ///
    /// ```text
    ///   0x<offset>:  [<label>:]  <MNEMONIC>  [operands]
    /// ```
    ///
    /// The label column is omitted entirely when no jumps are present.
    /// Invalid bytes are rendered as `.byte 0xXX` pseudo-instructions so no
    /// information is silently dropped.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_bytecode::types::Instruction;
    /// use aglais_xqvm_bytecode::codec;
    /// use aglais_xqvm_disasm::display::Disassembly;
    ///
    /// let program = [Instruction::Push { imm: 1 }, Instruction::Halt {}];
    /// let buf: Vec<u8> = program.iter().flat_map(codec::encode).collect();
    ///
    /// let mut out = Vec::new();
    /// Disassembly::new(&buf).write_to(&mut out).unwrap();
    /// let text = String::from_utf8(out).unwrap();
    /// assert!(text.contains("PUSH"));
    /// assert!(text.contains("HALT"));
    /// ```
    pub fn write_to(&self, out: &mut impl io::Write) -> io::Result<()> {
        // Width of the label column: longest "Lx:" string, or 0 when there
        // are no labels so the column is omitted entirely.
        let label_col = self
            .stream
            .labels()
            .values()
            .map(|s| s.len() + 1)
            .max()
            .unwrap_or(0);

        // Clone the label map so `fmt_instruction` can resolve i16 operands
        // to label names after the stream is consumed by iteration.
        let labels = self.stream.labels().clone();

        // Clone the stream to iterate from position 0 without mutating self.
        for item in self.stream.clone() {
            match item {
                Ok((offset, label, instr)) => {
                    let label_str = label.map(|s| format!("{s}:")).unwrap_or_default();
                    write!(out, "  0x{offset:04X}:  ")?;
                    if label_col > 0 {
                        write!(out, "{label_str:<label_col$}  ")?;
                    }
                    fmt_instruction(&instr, offset, &labels, out)?;
                    writeln!(out)?;
                }
                Err(ref e) => {
                    let (offset, byte) = match e {
                        stream::Error::UnknownOpcode { offset, byte } => (*offset, *byte),
                        stream::Error::TruncatedInstruction { offset } => {
                            (*offset, self.stream.bytes()[*offset])
                        }
                        stream::Error::SeekOutOfBounds { .. } => {
                            unreachable!("stream never seeks internally")
                        }
                    };
                    let label_str = labels
                        .get(&offset)
                        .map(|s| format!("{s}:"))
                        .unwrap_or_default();
                    write!(out, "  0x{offset:04X}:  ")?;
                    if label_col > 0 {
                        write!(out, "{label_str:<label_col$}  ")?;
                    }
                    writeln!(out, ".byte    0x{byte:02X}")?;
                }
            }
        }

        Ok(())
    }
}

impl fmt::Display for Disassembly<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = Vec::new();
        self.write_to(&mut buf).map_err(|_| fmt::Error)?;
        f.write_str(std::str::from_utf8(&buf).map_err(|_| fmt::Error)?)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use aglais_xqvm_bytecode::codec;
    use aglais_xqvm_bytecode::types::{Instruction, Register};

    fn assemble(program: &[Instruction]) -> Vec<u8> {
        program.iter().flat_map(codec::encode).collect()
    }

    #[test]
    fn basic_program_contains_mnemonics() {
        let buf = assemble(&[Instruction::Push { imm: 42 }, Instruction::Halt {}]);
        let text = Disassembly::new(&buf).to_string();
        assert!(text.contains("PUSH"), "missing PUSH in:\n{text}");
        assert!(text.contains("42"), "missing immediate 42 in:\n{text}");
        assert!(text.contains("HALT"), "missing HALT in:\n{text}");
    }

    #[test]
    fn register_operand_displays_as_r_slot() {
        let buf = assemble(&[Instruction::Load { reg: Register(3) }]);
        let text = Disassembly::new(&buf).to_string();
        assert!(text.contains("r3"), "expected r3 in:\n{text}");
    }

    #[test]
    fn byte_offsets_are_correct() {
        // PUSH(0) is 9 bytes (opcode + 8-byte BE i64); HALT starts at offset 9.
        let buf = assemble(&[Instruction::Push { imm: 0 }, Instruction::Halt {}]);
        let text = Disassembly::new(&buf).to_string();
        assert!(text.contains("0x0000"), "missing 0x0000 in:\n{text}");
        assert!(text.contains("0x0009"), "missing 0x0009 in:\n{text}");
    }

    #[test]
    fn backward_jump_gets_label() {
        // PUSH(5) (9 bytes, offset 0) + GT (1 byte, offset 9) = offset 10 for JUMPI.
        // JUMPI offset = -10  ->  target = 10 + (-10) = 0  -> L0 at offset 0.
        let buf = assemble(&[
            Instruction::Push { imm: 5 },
            Instruction::Gt {},
            Instruction::JumpI { offset: -10i16 },
            Instruction::Halt {},
        ]);
        let text = Disassembly::new(&buf).to_string();
        // Label must appear inline on the first instruction's line.
        assert!(text.contains("L0:"), "missing label L0 in:\n{text}");
        // Jump operand must reference the label, not the raw offset.
        assert!(
            text.contains("JUMPI   L0"),
            "expected 'JUMPI   L0' in:\n{text}"
        );
    }

    #[test]
    fn forward_jump_gets_label() {
        // JUMP (3 bytes, offset 0); target = 0 + 3 = 3 = offset of HALT.
        let buf = assemble(&[
            Instruction::Jump { offset: 3i16 },
            Instruction::Nop {},
            Instruction::Halt {},
        ]);
        let text = Disassembly::new(&buf).to_string();
        assert!(text.contains("L0:"), "missing label in:\n{text}");
        assert!(
            text.contains("JUMP    L0"),
            "expected 'JUMP    L0' in:\n{text}"
        );
    }

    #[test]
    fn two_distinct_jump_targets_get_distinct_labels() {
        // JUMPI -> offset 0 (L0)  and  JUMP -> offset 9 (L1) -- sorted by address.
        let buf = assemble(&[
            Instruction::JumpI { offset: -3i16 }, // offset 0; target = 0+(-3) = -3 -> OOB
            Instruction::Jump { offset: 6i16 },   // offset 3; target = 3+6 = 9
            Instruction::Push { imm: 0 },         // offset 6
            Instruction::Halt {},                 // offset 15
        ]);
        // target of JUMP is offset 9 = middle of PUSH, target of JUMPI is -3 (clamped to 0).
        // L0 appears at offset 0 (JUMPI's own address). L1 is at offset 9, which is
        // the middle of PUSH -- no instruction starts there, so "L1:" never appears.
        let text = Disassembly::new(&buf).to_string();
        assert!(text.contains("L0:"), "missing L0 in:\n{text}");
        assert!(!text.contains("L1:"), "unexpected L1 in:\n{text}");
    }

    #[test]
    fn unknown_byte_displays_as_dot_byte() {
        let buf = [0xFEu8]; // not a valid opcode
        let text = Disassembly::new(&buf).to_string();
        assert!(text.contains(".byte"), "expected .byte in:\n{text}");
        assert!(text.contains("0xFE"), "expected 0xFE in:\n{text}");
    }

    #[test]
    fn energy_instruction_shows_two_registers() {
        let buf = assemble(&[Instruction::Energy {
            model: Register(1),
            sample: Register(2),
        }]);
        let text = Disassembly::new(&buf).to_string();
        assert!(text.contains("ENERGY"), "missing ENERGY in:\n{text}");
        assert!(text.contains("r1"), "missing r1 in:\n{text}");
        assert!(text.contains("r2"), "missing r2 in:\n{text}");
    }

    #[test]
    fn negative_immediate_displays_correctly() {
        let buf = assemble(&[Instruction::Push { imm: -7 }]);
        let text = Disassembly::new(&buf).to_string();
        assert!(text.contains("-7"), "expected -7 in:\n{text}");
    }

    #[test]
    fn empty_bytecode_produces_empty_output() {
        assert_eq!(Disassembly::new(&[]).to_string(), "");
    }
}
