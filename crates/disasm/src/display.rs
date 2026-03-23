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
//! The entry point is [`Disassembly`], which wraps either a raw byte slice or
//! a complete [`Program`] and
//! implements [`Display`](std::fmt::Display). Jump targets are automatically
//! assigned sequential labels (`L0`, `L1`, ...) and rendered both at the
//! destination and as the operand of the originating `JUMP`/`JUMPI`.
//!
//! `PUSHC_1`..`PUSHC_8` operands are rendered as their sign-extended decimal
//! value.
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
//!   0x0000:  L0:  PUSHC_2  12345
//!   0x0003:       GT
//!   0x0004:       JUMPI    L0
//!   0x0007:       HALT
//! ```
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_bytecode::{Instruction, codec};
//! use aglais_xqvm_disasm::Disassembly;
//!
//! // No jumps -- label column is suppressed entirely.
//! let program = [
//!     Instruction::PushC1 { val: [1] },
//!     Instruction::PushC1 { val: [2] },
//!     Instruction::Add  {},
//!     Instruction::Halt {},
//! ];
//! let buf: Vec<u8> = program.iter().flat_map(|i| codec::encode(i)).collect();
//!
//! let text = Disassembly::new(&buf).to_string();
//! assert!(text.contains("PUSHC_1"));
//! assert!(text.contains("ADD"));
//! assert!(text.contains("HALT"));
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::io;

use aglais_xqvm_bytecode::error::StreamError;
use aglais_xqvm_bytecode::{Instruction, InstructionStream, Program, Register, opcodes};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Sign-extend a big-endian byte slice (1..=8 bytes) to `i64`.
fn sign_extend_be(bytes: &[u8]) -> i64 {
    debug_assert!(!bytes.is_empty() && bytes.len() <= 8);
    let mut v = 0i64;
    for &b in bytes {
        v = (v << 8) | i64::from(b);
    }
    let shift = 64u32 - (bytes.len() * 8) as u32;
    (v << shift) >> shift
}

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

/// `[u8; N]` fields appear on `PUSHC_1`..`PUSHC_8` -- render as a decimal
/// by sign-extending the bytes to `i64`.
macro_rules! impl_fmt_operand_byte_array {
    ($($n:literal),+) => {
        $(
            impl FmtOperand for [u8; $n] {
                fn fmt_operand(
                    &self,
                    f: &mut dyn io::Write,
                    _labels: &BTreeMap<usize, String>,
                    _instr_offset: usize,
                ) -> io::Result<()> {
                    write!(f, "{}", sign_extend_be(self))
                }
            }
        )+
    };
}

impl_fmt_operand_byte_array!(1, 2, 3, 4, 5, 6, 7, 8);

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

/// A pretty-printed view of raw XQVM bytecode or a complete [`Program`].
///
/// Jump targets are assigned labels `L0`, `L1`, ... in address order by the
/// underlying [`InstructionStream`]. Each label is printed inline between the
/// byte offset and the mnemonic. When no label exists at an address the
/// column is left blank so all columns align. `JUMP`/`JUMPI` operands are
/// replaced by the label name when the target is within the same buffer.
/// `PUSHC_1`..`PUSHC_8` operands are sign-extended and rendered as decimals.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_bytecode::{Instruction, Register, codec};
/// use aglais_xqvm_disasm::Disassembly;
///
/// // Build a trivial counted-down loop.
/// let program = [
///     Instruction::PushC1 { val: [5] },        // push counter
///     Instruction::PushC1 { val: [0xFF] },     // push decrement (-1)
///     Instruction::Target {},                   // loop target
///     Instruction::Add    {},                   // counter--
///     Instruction::Dupl   {},                   // check without consuming
///     Instruction::JumpI  { offset: -4i16 },   // loop back if non-zero
///     Instruction::Pop    {},
///     Instruction::Halt   {},
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

    /// Wrap a [`Program`] for display.
    pub fn from_program(program: &'a Program) -> Self {
        Self {
            stream: InstructionStream::from_program(program),
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
    /// use aglais_xqvm_bytecode::{Instruction, codec};
    /// use aglais_xqvm_disasm::Disassembly;
    ///
    /// let program = [Instruction::PushC1 { val: [1] }, Instruction::Halt {}];
    /// let buf: Vec<u8> = program.iter().flat_map(codec::encode).collect();
    ///
    /// let mut out = Vec::new();
    /// Disassembly::new(&buf).write_to(&mut out).unwrap();
    /// let text = String::from_utf8(out).unwrap();
    /// assert!(text.contains("PUSHC_1"));
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
                        StreamError::UnknownOpcode { offset, byte } => (*offset, *byte),
                        StreamError::TruncatedInstruction { offset } => {
                            let byte = *self.stream.bytes().get(*offset).unwrap_or_else(|| {
                                unreachable!("TruncatedInstruction offset within buffer")
                            });
                            (*offset, byte)
                        }
                        StreamError::SeekOutOfBounds { .. } => {
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
    use aglais_xqvm_bytecode::{Instruction, Register, codec};

    fn assemble(program: &[Instruction]) -> Vec<u8> {
        program.iter().flat_map(codec::encode).collect()
    }

    #[test]
    fn basic_program_contains_mnemonics() {
        let buf = assemble(&[Instruction::PushC1 { val: [42] }, Instruction::Halt {}]);
        let text = Disassembly::new(&buf).to_string();
        assert!(text.contains("PUSHC_1"), "missing PUSHC_1 in:\n{text}");
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
        // PUSHC_0 is 1 byte (opcode only); HALT starts at offset 1.
        let buf = assemble(&[Instruction::PushC0 {}, Instruction::Halt {}]);
        let text = Disassembly::new(&buf).to_string();
        assert!(text.contains("0x0000"), "missing 0x0000 in:\n{text}");
        assert!(text.contains("0x0001"), "missing 0x0001 in:\n{text}");
    }

    #[test]
    fn backward_jump_gets_label() {
        // PUSHC_1 [5] (2 bytes, offset 0) + GT (1 byte, offset 2) = offset 3 for JUMPI.
        // JUMPI offset = -3  ->  target = 3 + (-3) = 0  -> L0 at offset 0.
        let buf = assemble(&[
            Instruction::PushC1 { val: [5] },
            Instruction::Gt {},
            Instruction::JumpI { offset: -3i16 },
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
        // JUMPI -> offset 0 (L0)  and  JUMP -> offset 7 (L1) -- sorted by address.
        let buf = assemble(&[
            Instruction::JumpI { offset: -3i16 }, // offset 0; target = 0+(-3) = -3 -> OOB
            Instruction::Jump { offset: 4i16 },   // offset 3; target = 3+4 = 7
            Instruction::PushC8 {
                // offset 6 (9 bytes: 6-14)
                val: [0, 0, 0, 0, 0, 0, 0, 0],
            },
            Instruction::Halt {}, // offset 15
        ]);
        // target of JUMP is offset 7 = inside PUSHC_8 (starts at 6, 9 bytes), target of
        // JUMPI is -3 (out of bounds). L0 appears at offset 0 (JUMPI's own address).
        // L1 is at offset 7, which is inside PUSHC_8 -- no instruction starts there,
        // so "L1:" never appears.
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
        let buf = assemble(&[Instruction::PushC1 { val: [0xF9] }]); // -7 as i8
        let text = Disassembly::new(&buf).to_string();
        assert!(text.contains("-7"), "expected -7 in:\n{text}");
    }

    #[test]
    fn empty_bytecode_produces_empty_output() {
        assert_eq!(Disassembly::new(&[]).to_string(), "");
    }

    #[test]
    fn pushc2_displays_value() {
        // PUSHC_2 with 0x0007 encodes the value 7.
        let buf = assemble(&[
            Instruction::PushC2 { val: [0x00, 0x07] },
            Instruction::Halt {},
        ]);
        let text = Disassembly::new(&buf).to_string();
        assert!(text.contains("PUSHC_2"), "missing PUSHC_2 in:\n{text}");
        assert!(text.contains('7'), "missing value 7 in:\n{text}");
    }

    #[test]
    fn pushc3_displays_large_value() {
        // 100000 = 0x0001_86A0 fits in PUSHC_3.
        let buf = assemble(&[
            Instruction::PushC3 {
                val: [0x01, 0x86, 0xA0],
            },
            Instruction::Halt {},
        ]);
        let text = Disassembly::new(&buf).to_string();
        assert!(text.contains("100000"), "missing 100000 in:\n{text}");
    }
}
