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

//! Incremental, seekable reader for XQVM bytecode.
//!
//! [`InstructionStream`] wraps a raw byte buffer and decodes one instruction
//! at a time, advancing an internal cursor after each successful read.
//! Seeking moves the cursor to any absolute byte offset, which lets the VM
//! execute `JUMP`/`JUMPI` targets directly without re-scanning the buffer.
//!
//! During construction the stream performs a single scan to collect all
//! `JUMP`/`JUMPI` targets and assign them sequential labels `L0`, `L1`, ...
//! in address order. These labels are then included in every decoded
//! instruction tuple so that consumers (a disassembler, a debugger, a
//! tracing VM) do not need to perform a separate pre-pass.
//!
//! The stream also implements [`Iterator`] so it can be consumed with
//! standard iterator combinators.
//!
//! # Errors
//!
//! [`enum@Error`] is returned when a byte cannot be decoded or a seek is out of
//! bounds. On decode errors the cursor advances by one byte so the stream
//! always makes progress and infinite loops in consumer code are prevented.
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_bytecode::types::Instruction;
//! use aglais_xqvm_bytecode::{codec, stream::InstructionStream};
//!
//! let program = [
//!     Instruction::Push { imm: 1 },
//!     Instruction::Push { imm: 2 },
//!     Instruction::Add  {},
//!     Instruction::Halt {},
//! ];
//! let buf: Vec<u8> = program.iter().flat_map(|i| codec::encode(i)).collect();
//!
//! // No jumps -- every label is None.
//! let decoded: Vec<_> = InstructionStream::new(&buf)
//!     .collect::<std::result::Result<Vec<_>, _>>()
//!     .unwrap();
//!
//! assert_eq!(decoded.len(), 4);
//! assert_eq!(decoded[0].2, Instruction::Push { imm: 1 });
//! assert_eq!(decoded[3].2, Instruction::Halt {});
//! assert!(decoded.iter().all(|(_, label, _)| label.is_none()));
//! ```

use std::collections::BTreeMap;

use thiserror::Error;

use crate::codec;
use crate::types::{Instruction, Opcode};

// ---------------------------------------------------------------------------
// Error and Result
// ---------------------------------------------------------------------------

/// All errors produced by [`InstructionStream`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    /// The byte at `offset` is not a recognised XQVM opcode.
    ///
    /// The stream cursor advances past the offending byte so iteration always
    /// makes progress.
    #[error("unknown opcode 0x{byte:02X} at offset {offset:#06X}")]
    UnknownOpcode {
        /// Absolute byte offset of the bad opcode byte.
        offset: usize,
        /// The unrecognised byte value.
        byte: u8,
    },

    /// The buffer ends before all operand bytes of the instruction at
    /// `offset` have been read.
    ///
    /// The stream cursor advances past the offending byte so iteration always
    /// makes progress.
    #[error("truncated instruction at offset {offset:#06X}")]
    TruncatedInstruction {
        /// Absolute byte offset of the opcode whose operands are missing.
        offset: usize,
    },

    /// The seek target exceeds the length of the underlying buffer.
    #[error("seek target {target} is out of bounds (buffer length {len})")]
    SeekOutOfBounds {
        /// The requested byte offset.
        target: usize,
        /// Total length of the underlying buffer.
        len: usize,
    },
}

/// Convenience alias for `std::result::Result<T, `[`enum@Error`]`>`.
pub type Result<T> = std::result::Result<T, Error>;

// ---------------------------------------------------------------------------
// Label collection (single pre-pass over the raw bytes)
// ---------------------------------------------------------------------------

/// Scan `bytes` once, find all `JUMP`/`JUMPI` targets, and assign each a
/// stable sequential label name (`L0`, `L1`, ...) in ascending address order.
pub(crate) fn collect_labels(bytes: &[u8]) -> BTreeMap<usize, String> {
    let mut pos = 0usize;
    let mut targets: Vec<usize> = Vec::new();

    while pos < bytes.len() {
        match codec::decode(&bytes[pos..]) {
            Ok((instr, consumed)) => {
                let rel = match &instr {
                    Instruction::Jump { offset: r } | Instruction::JumpI { offset: r } => *r,
                    _ => {
                        pos += consumed;
                        continue;
                    }
                };
                let target = (pos as i64 + i64::from(rel)).max(0) as usize;
                targets.push(target);
                pos += consumed;
            }
            Err(_) => pos += 1,
        }
    }

    targets.sort_unstable();
    targets.dedup();

    targets
        .into_iter()
        .enumerate()
        .map(|(i, addr)| (addr, format!("L{i}")))
        .collect()
}

// ---------------------------------------------------------------------------
// InstructionStream
// ---------------------------------------------------------------------------

/// Incremental, seekable reader over a raw XQVM bytecode buffer.
///
/// The stream holds a shared reference to a byte slice, an internal cursor,
/// and a pre-computed label map. Calling
/// [`next_instruction`](InstructionStream::next_instruction) (or iterating)
/// decodes the next instruction at the current cursor position, advances the
/// cursor, and returns the optional label assigned to that address.
///
/// [`seek`](InstructionStream::seek) moves the cursor to any absolute byte
/// offset in `[0, len]`. Seeking to `len` is valid and positions the stream
/// past the end so the next read returns `None`.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_bytecode::types::Instruction;
/// use aglais_xqvm_bytecode::{codec, stream::InstructionStream};
///
/// // Build a short loop: PUSH 3 / JUMPI -9 (back to PUSH at offset 0).
/// let program = [
///     Instruction::Push  { imm: 3  },        // offset 0 (9 bytes), target of JUMPI -> label L0
///     Instruction::JumpI { offset: -9i16 },  // offset 9; target = 9 + (-9) = 0
/// ];
/// let buf: Vec<u8> = program.iter().flat_map(|i| codec::encode(i)).collect();
///
/// let mut stream = InstructionStream::new(&buf);
///
/// // PUSH at offset 0 is the jump target, so it carries label "L0".
/// let (off0, label0, instr0) = stream.next_instruction().unwrap().unwrap();
/// assert_eq!(off0, 0);
/// assert_eq!(label0.as_deref(), Some("L0"));
/// assert_eq!(instr0, Instruction::Push { imm: 3 });
///
/// // JUMPI has no label at its own address.
/// let (off1, label1, instr1) = stream.next_instruction().unwrap().unwrap();
/// assert_eq!(off1, 9);
/// assert_eq!(label1, None);
/// assert_eq!(instr1, Instruction::JumpI { offset: -9i16 });
///
/// // Execute the jump: seek back to the label address.
/// stream.seek(off0).unwrap();
/// assert_eq!(stream.pos(), 0);
/// ```
#[derive(Debug, Clone)]
pub struct InstructionStream<'a> {
    bytes: &'a [u8],
    pos: usize,
    labels: BTreeMap<usize, String>,
}

impl<'a> InstructionStream<'a> {
    /// Create a new stream positioned at the start of `bytes`.
    ///
    /// The constructor performs a single scan of `bytes` to collect all
    /// `JUMP`/`JUMPI` targets and assign label names before any instruction
    /// is returned.
    pub fn new(bytes: &'a [u8]) -> Self {
        let labels = collect_labels(bytes);
        Self {
            bytes,
            pos: 0,
            labels,
        }
    }

    /// Current cursor position (byte offset into the buffer).
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Total length of the underlying byte buffer.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns `true` if the buffer contains no bytes.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The raw byte slice underlying this stream.
    pub fn bytes(&self) -> &[u8] {
        self.bytes
    }

    /// The label map computed during construction.
    ///
    /// Keys are absolute byte offsets; values are label names (`"L0"`,
    /// `"L1"`, ...) assigned in ascending address order. Labels are derived
    /// exclusively from `JUMP`/`JUMPI` targets.
    pub fn labels(&self) -> &BTreeMap<usize, String> {
        &self.labels
    }

    /// Seek to absolute byte offset `pos`.
    ///
    /// `pos` may be anywhere in `[0, len]`. Seeking to `len` positions the
    /// stream just past the last byte; the next [`next_instruction`](Self::next_instruction) call
    /// will return `None`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::SeekOutOfBounds`] if `pos > len`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_bytecode::types::Instruction;
    /// use aglais_xqvm_bytecode::{codec, stream::{Error, InstructionStream}};
    ///
    /// let buf = codec::encode(&Instruction::Nop {});
    /// let mut stream = InstructionStream::new(&buf);
    ///
    /// assert!(stream.seek(0).is_ok());
    /// assert!(stream.seek(buf.len()).is_ok()); // seek to end is valid
    /// assert_eq!(
    ///     stream.seek(buf.len() + 1),
    ///     Err(Error::SeekOutOfBounds { target: buf.len() + 1, len: buf.len() }),
    /// );
    /// ```
    pub fn seek(&mut self, pos: usize) -> Result<()> {
        if pos > self.bytes.len() {
            return Err(Error::SeekOutOfBounds {
                target: pos,
                len: self.bytes.len(),
            });
        }
        self.pos = pos;
        Ok(())
    }

    /// Decode and return the next instruction, advancing the cursor.
    ///
    /// Returns:
    /// - `None` when the cursor is at or past the end of the buffer.
    /// - `Some(Ok((offset, label, instruction)))` on a successful decode,
    ///   where `offset` is the byte position of the opcode and `label` is the
    ///   name assigned to that address (e.g. `Some("L0")`), or `None` when
    ///   no jump targets this address.
    /// - `Some(Err(e))` when the bytes at the cursor cannot be decoded.
    ///   The cursor advances by one byte so subsequent calls make progress.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_bytecode::types::Instruction;
    /// use aglais_xqvm_bytecode::{codec, stream::{Error, InstructionStream}};
    ///
    /// // Valid instruction followed by an unknown byte.
    /// // 0x08 is a gap opcode (not assigned).
    /// let mut buf = codec::encode(&Instruction::Nop {});
    /// buf.push(0x08u8);
    ///
    /// let mut stream = InstructionStream::new(&buf);
    ///
    /// assert_eq!(
    ///     stream.next_instruction(),
    ///     Some(Ok((0, None, Instruction::Nop {}))),
    /// );
    /// assert_eq!(
    ///     stream.next_instruction(),
    ///     Some(Err(Error::UnknownOpcode { offset: 1, byte: 0x08 })),
    /// );
    /// assert_eq!(stream.next_instruction(), None);
    /// ```
    pub fn next_instruction(&mut self) -> Option<Result<(usize, Option<String>, Instruction)>> {
        if self.pos >= self.bytes.len() {
            return None;
        }

        let offset = self.pos;
        let label = self.labels.get(&offset).cloned();

        match codec::decode(&self.bytes[offset..]) {
            Ok((instr, consumed)) => {
                self.pos += consumed;
                Some(Ok((offset, label, instr)))
            }
            Err(e) => {
                // Always advance by one so the stream makes progress.
                self.pos += 1;
                Some(Err(map_decode_error(e, offset, self.bytes[offset])))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Iterator
// ---------------------------------------------------------------------------

impl Iterator for InstructionStream<'_> {
    type Item = Result<(usize, Option<String>, Instruction)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_instruction()
    }
}

// ---------------------------------------------------------------------------
// Error mapping
// ---------------------------------------------------------------------------

fn map_decode_error(_err: oxicode::Error, offset: usize, byte: u8) -> Error {
    // oxicode serde wraps all errors (including UnexpectedEnd) into
    // Custom { message } before returning from decode_from_slice, so we cannot
    // distinguish truncation from unknown opcode by inspecting the error value.
    // Instead: if the first byte is a known opcode, the buffer must be too short
    // (truncated operands); otherwise the byte itself is the problem.
    if Opcode::try_from(byte).is_ok() {
        Error::TruncatedInstruction { offset }
    } else {
        Error::UnknownOpcode { offset, byte }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Instruction, Register};

    fn assemble(program: &[Instruction]) -> Vec<u8> {
        program.iter().flat_map(codec::encode).collect()
    }

    #[test]
    fn empty_buffer_yields_none_immediately() {
        let mut stream = InstructionStream::new(&[]);
        assert_eq!(stream.next_instruction(), None);
    }

    #[test]
    fn single_instruction_roundtrip() {
        let buf = assemble(&[Instruction::Halt {}]);
        let mut stream = InstructionStream::new(&buf);
        assert_eq!(
            stream.next_instruction(),
            Some(Ok((0, None, Instruction::Halt {}))),
        );
        assert_eq!(stream.next_instruction(), None);
    }

    #[test]
    fn offsets_advance_correctly() {
        // PUSH(0) (9 bytes) at 0, HALT (1 byte) at 9.
        let buf = assemble(&[Instruction::Push { imm: 0 }, Instruction::Halt {}]);
        let mut stream = InstructionStream::new(&buf);

        let (off0, _, _) = stream.next_instruction().unwrap().unwrap();
        let (off1, _, _) = stream.next_instruction().unwrap().unwrap();
        assert_eq!(off0, 0);
        assert_eq!(off1, 9);
        assert_eq!(stream.next_instruction(), None);
    }

    #[test]
    fn iterator_collects_all_instructions() {
        let program = [
            Instruction::Push { imm: 5 },
            Instruction::Push { imm: 3 },
            Instruction::Add {},
            Instruction::Halt {},
        ];
        let buf = assemble(&program);
        let items: Vec<_> = InstructionStream::new(&buf)
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(items.len(), 4);
        for (i, (_, _, instr)) in items.iter().enumerate() {
            assert_eq!(*instr, program[i]);
        }
    }

    #[test]
    fn jump_target_instruction_carries_label() {
        // PUSH(3) (9 bytes) at 0, JUMPI (3 bytes) at 9; target = 9 + (-9) = 0 -> L0.
        let buf = assemble(&[
            Instruction::Push { imm: 3 },
            Instruction::JumpI { offset: -9i16 },
        ]);
        let mut stream = InstructionStream::new(&buf);

        let (_, label0, _) = stream.next_instruction().unwrap().unwrap();
        let (_, label1, _) = stream.next_instruction().unwrap().unwrap();

        assert_eq!(label0.as_deref(), Some("L0"), "PUSH at target should be L0");
        assert_eq!(label1, None, "JUMPI itself has no label");
    }

    #[test]
    fn no_labels_when_no_jumps() {
        let buf = assemble(&[Instruction::Push { imm: 1 }, Instruction::Halt {}]);
        let stream = InstructionStream::new(&buf);
        assert!(stream.labels().is_empty());
    }

    #[test]
    fn labels_map_has_correct_entries() {
        // JUMP (3 bytes) at 0; target = 0 + 4 = 4 -> L0.
        // NOP (1 byte) at 3.
        // HALT (1 byte) at 4 -> labeled L0.
        let buf = assemble(&[
            Instruction::Jump { offset: 4i16 },
            Instruction::Nop {},
            Instruction::Halt {},
        ]);
        let stream = InstructionStream::new(&buf);
        let labels = stream.labels();
        assert_eq!(labels.len(), 1);
        assert_eq!(labels.get(&4).map(String::as_str), Some("L0"));
    }

    #[test]
    fn seek_to_second_instruction_skips_first() {
        // PUSH(0) (9 bytes) at 0, NOP (1 byte) at 9, HALT (1 byte) at 10.
        let buf = assemble(&[
            Instruction::Push { imm: 0 },
            Instruction::Nop {},
            Instruction::Halt {},
        ]);
        let mut stream = InstructionStream::new(&buf);

        stream.seek(9).unwrap();
        let (off, _, instr) = stream.next_instruction().unwrap().unwrap();
        assert_eq!(off, 9);
        assert_eq!(instr, Instruction::Nop {});
    }

    #[test]
    fn seek_to_end_makes_stream_done() {
        let buf = assemble(&[Instruction::Halt {}]);
        let mut stream = InstructionStream::new(&buf);
        stream.seek(buf.len()).unwrap();
        assert_eq!(stream.next_instruction(), None);
    }

    #[test]
    fn seek_out_of_bounds_returns_error() {
        let buf = assemble(&[Instruction::Halt {}]);
        let mut stream = InstructionStream::new(&buf);
        assert_eq!(
            stream.seek(buf.len() + 1),
            Err(Error::SeekOutOfBounds {
                target: buf.len() + 1,
                len: buf.len()
            }),
        );
        // Position is unchanged after a failed seek.
        assert_eq!(stream.pos(), 0);
    }

    #[test]
    fn seek_back_simulates_jump() {
        // PUSH(3) (9 bytes) at 0, JUMPI (3 bytes) at 9; target = 9 + (-9) = 0.
        let buf = assemble(&[
            Instruction::Push { imm: 3 },
            Instruction::JumpI { offset: -9i16 },
        ]);
        let mut stream = InstructionStream::new(&buf);

        let (_, _, _push) = stream.next_instruction().unwrap().unwrap();
        let (off_jumpi, _, jumpi) = stream.next_instruction().unwrap().unwrap();

        // Compute jump target as the VM would.
        let target = if let Instruction::JumpI { offset } = jumpi {
            (off_jumpi as i64 + i64::from(offset)) as usize
        } else {
            panic!("expected JUMPI");
        };

        stream.seek(target).unwrap();
        let (off, label, instr) = stream.next_instruction().unwrap().unwrap();
        assert_eq!(off, 0);
        assert_eq!(label.as_deref(), Some("L0"));
        assert_eq!(instr, Instruction::Push { imm: 3 });
    }

    #[test]
    fn unknown_opcode_returns_error_and_advances() {
        // 0x08 is a gap opcode (not assigned).
        let buf = [0x08u8, Instruction::Halt {}.opcode() as u8];
        let mut stream = InstructionStream::new(&buf);

        assert_eq!(
            stream.next_instruction(),
            Some(Err(Error::UnknownOpcode {
                offset: 0,
                byte: 0x08
            })),
        );
        // After the error the cursor moved to byte 1; HALT should decode.
        assert_eq!(
            stream.next_instruction(),
            Some(Ok((1, None, Instruction::Halt {}))),
        );
    }

    #[test]
    fn truncated_instruction_returns_error_and_advances() {
        // PUSH opcode only, no operand bytes -- truncated.
        let buf = [0x10u8];
        let mut stream = InstructionStream::new(&buf);

        assert_eq!(
            stream.next_instruction(),
            Some(Err(Error::TruncatedInstruction { offset: 0 })),
        );
        // Cursor is at 1 == len, so stream is exhausted.
        assert_eq!(stream.next_instruction(), None);
    }

    #[test]
    fn pos_and_len_are_correct() {
        let buf = assemble(&[Instruction::Nop {}, Instruction::Halt {}]);
        let mut stream = InstructionStream::new(&buf);
        assert_eq!(stream.len(), 2);
        assert_eq!(stream.pos(), 0);
        let _ = stream.next_instruction();
        assert_eq!(stream.pos(), 1);
        let _ = stream.next_instruction();
        assert_eq!(stream.pos(), 2);
    }

    #[test]
    fn register_instruction_decodes_correctly() {
        let buf = assemble(&[Instruction::Load { reg: Register(7) }]);
        let mut stream = InstructionStream::new(&buf);
        let (_, _, instr) = stream.next_instruction().unwrap().unwrap();
        assert_eq!(instr, Instruction::Load { reg: Register(7) });
    }
}
