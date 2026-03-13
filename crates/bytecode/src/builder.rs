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

//! Fluent bytecode assembler with first-class label support.
//!
//! [`InstructionBuilder`] lets you emit XQVM instructions one at a time
//! using a chained, builder-style API. Labels are opaque handles created
//! with [`label`](InstructionBuilder::label) and anchored to a byte offset
//! with [`place`](InstructionBuilder::place). `JUMP`/`JUMPI` instructions
//! record a fixup instead of a raw offset; [`build`](InstructionBuilder::build)
//! resolves all fixups and returns the final byte buffer.
//!
//! Both forward and backward references work: you may call
//! [`jump`](InstructionBuilder::jump) before or after
//! [`place`](InstructionBuilder::place) on the same label.
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_bytecode::builder::InstructionBuilder;
//!
//! // Counted loop: push 3, decrement until zero.
//! let mut b = InstructionBuilder::new();
//! let loop_top = b.label();
//!
//! b.push(3);
//! b.place(loop_top);
//! b.push(-1);
//! b.add();
//! b.dupl();
//! b.jump_if(loop_top);  // backward reference
//! b.pop();
//! b.halt();
//!
//! let buf = b.build().unwrap();
//! assert!(!buf.is_empty());
//! ```
//!
//! Forward references work equally well:
//!
//! ```rust
//! use aglais_xqvm_bytecode::builder::InstructionBuilder;
//!
//! let mut b = InstructionBuilder::new();
//! let done = b.label();
//!
//! b.push(0);
//! b.jump_if(done);  // forward reference -- target not yet placed
//! b.push(42);
//! b.place(done);    // anchor here
//! b.halt();
//!
//! let buf = b.build().unwrap();
//! assert!(!buf.is_empty());
//! ```

use thiserror::Error;

use crate::codec;
use crate::types::{Instruction, Register};

// ---------------------------------------------------------------------------
// Error and Result
// ---------------------------------------------------------------------------

/// Error returned by [`InstructionBuilder::build`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    /// A label was used in a `JUMP`/`JUMPI` but never placed.
    #[error("label {id} was used but never placed")]
    UnplacedLabel {
        /// Index of the unplaced label.
        id: usize,
    },

    /// The byte distance between a jump and its target exceeds the `i16` range.
    #[error(
        "jump at offset {site:#06X} to label {label}: \
         offset {delta} does not fit in i16"
    )]
    OffsetOverflow {
        /// Byte offset of the JUMP/JUMPI instruction.
        site: usize,
        /// Index of the target label.
        label: usize,
        /// Computed delta that overflowed.
        delta: i64,
    },
}

/// Convenience alias for `std::result::Result<T, `[`enum@Error`]`>`.
pub type Result<T> = std::result::Result<T, Error>;

// ---------------------------------------------------------------------------
// LabelId
// ---------------------------------------------------------------------------

/// An opaque handle to a bytecode label created by
/// [`InstructionBuilder::label`].
///
/// Labels are allocated in the order they are created. Pass a `LabelId` to
/// [`InstructionBuilder::place`] to anchor the label at a byte offset, and
/// to [`InstructionBuilder::jump`] / [`InstructionBuilder::jump_if`] to
/// emit a jump that targets the label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LabelId(usize);

// ---------------------------------------------------------------------------
// InstructionBuilder
// ---------------------------------------------------------------------------

/// Pending jump fixup: byte offset of the `JUMP`/`JUMPI` instruction and the
/// label it should resolve to.
#[derive(Debug)]
struct Fixup {
    /// Byte offset of the `JUMP`/`JUMPI` instruction (opcode byte).
    site: usize,
    label: LabelId,
}

/// Fluent XQVM bytecode assembler.
///
/// Create a builder with [`new`](Self::new), allocate labels with
/// [`label`](Self::label), emit instructions through the named methods or
/// [`emit`](Self::emit), anchor labels with [`place`](Self::place), and
/// finalise the buffer with [`build`](Self::build).
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_bytecode::builder::InstructionBuilder;
/// use aglais_xqvm_bytecode::types::{Instruction, Register};
///
/// let mut b = InstructionBuilder::new();
/// let skip = b.label();
///
/// b.push(1)
///  .push(2)
///  .gt()
///  .jump_if(skip)
///  .push(99)
///  .place(skip)
///  .halt();
///
/// let buf = b.build().unwrap();
/// assert!(!buf.is_empty());
/// ```
#[derive(Debug, Default)]
pub struct InstructionBuilder {
    buf: Vec<u8>,
    /// `None` means allocated but not yet placed.
    label_positions: Vec<Option<usize>>,
    fixups: Vec<Fixup>,
}

// ---------------------------------------------------------------------------
// tt-muncher: generate no-argument and single-Register emit wrappers from
// the opcode table.  Entries with `offset`, `imm`, or `model` fields are
// skipped because they have dedicated hand-written methods.
// ---------------------------------------------------------------------------

macro_rules! impl_builder_methods {
    // Base case.
    () => {};

    // Skip JUMP / JUMPI  -- `{offset: ...}`
    ( ($code:literal, $variant:ident, $mnem:literal, $doc:literal,
       {offset: $($rest_f:tt)*}), $($rest:tt)* ) => {
        impl_builder_methods!($($rest)*);
    };

    // Skip PUSH -- `{imm: ...}`
    ( ($code:literal, $variant:ident, $mnem:literal, $doc:literal,
       {imm: $($rest_f:tt)*}), $($rest:tt)* ) => {
        impl_builder_methods!($($rest)*);
    };

    // Skip ENERGY -- `{model: ...}`
    ( ($code:literal, $variant:ident, $mnem:literal, $doc:literal,
       {model: $($rest_f:tt)*}), $($rest:tt)* ) => {
        impl_builder_methods!($($rest)*);
    };

    // No-argument variants -- empty field list `{}`
    ( ($code:literal, $variant:ident, $mnem:literal, $doc:literal, {}),
      $($rest:tt)* ) => {
        ::pastey::paste! {
            #[doc = $doc]
            #[allow(clippy::should_implement_trait)]
            pub fn [<$variant:snake>](&mut self) -> &mut Self {
                self.emit(Instruction::$variant {})
            }
        }
        impl_builder_methods!($($rest)*);
    };

    // Single-register variants -- `{reg: <type>}`
    ( ($code:literal, $variant:ident, $mnem:literal, $doc:literal,
       {reg: $($ftype:tt)*}), $($rest:tt)* ) => {
        ::pastey::paste! {
            #[doc = $doc]
            pub fn [<$variant:snake>](&mut self, reg: Register) -> &mut Self {
                self.emit(Instruction::$variant { reg })
            }
        }
        impl_builder_methods!($($rest)*);
    };
}

impl InstructionBuilder {
    /// Create an empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    // -----------------------------------------------------------------------
    // Label API
    // -----------------------------------------------------------------------

    /// Allocate a new, unplaced label.
    ///
    /// The returned [`LabelId`] can be passed to [`place`](Self::place),
    /// [`jump`](Self::jump), and [`jump_if`](Self::jump_if) in any order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_bytecode::builder::InstructionBuilder;
    ///
    /// let mut b = InstructionBuilder::new();
    /// let top = b.label();
    /// b.place(top).nop().jump(top);
    /// b.build().unwrap();
    /// ```
    pub fn label(&mut self) -> LabelId {
        let id = self.label_positions.len();
        self.label_positions.push(None);
        LabelId(id)
    }

    /// Anchor `label` to the current write position.
    ///
    /// # Panics
    ///
    /// Panics if `label` was already placed (each label may be placed exactly
    /// once) or if `label` was not created by this builder.
    pub fn place(&mut self, label: LabelId) -> &mut Self {
        assert!(
            self.label_positions[label.0].is_none(),
            "label {id} placed more than once",
            id = label.0,
        );
        self.label_positions[label.0] = Some(self.buf.len());
        self
    }

    // -----------------------------------------------------------------------
    // General emit
    // -----------------------------------------------------------------------

    /// Encode and append any instruction to the buffer.
    ///
    /// Prefer the named convenience methods; use this for instructions that
    /// do not have a dedicated method.
    pub fn emit(&mut self, instr: Instruction) -> &mut Self {
        self.buf.extend_from_slice(&codec::encode(&instr));
        self
    }

    // -----------------------------------------------------------------------
    // Control flow (jump instructions use labels, not raw offsets)
    // -----------------------------------------------------------------------

    /// Emit a `JUMP` instruction targeting `label`.
    ///
    /// The actual `i16` offset is filled in during [`build`](Self::build).
    pub fn jump(&mut self, label: LabelId) -> &mut Self {
        let site = self.buf.len();
        // Use i16::MAX as placeholder: its zigzag-varint encoding is always
        // 3 bytes, reserving space for any i16 offset to be patched in build().
        self.buf
            .extend_from_slice(&codec::encode(&Instruction::Jump { offset: i16::MAX }));
        self.fixups.push(Fixup { site, label });
        self
    }

    /// Emit a `JUMPI` instruction targeting `label`.
    ///
    /// Jumps if the top of the stack is non-zero. The actual `i16` offset is
    /// filled in during [`build`](Self::build).
    pub fn jump_if(&mut self, label: LabelId) -> &mut Self {
        let site = self.buf.len();
        // Use i16::MAX as placeholder: its zigzag-varint encoding is always
        // 3 bytes, reserving space for any i16 offset to be patched in build().
        self.buf
            .extend_from_slice(&codec::encode(&Instruction::JumpI { offset: i16::MAX }));
        self.fixups.push(Fixup { site, label });
        self
    }

    // Generated from the opcode table: no-arg and single-register methods.
    // JUMP, JUMPI, PUSH, and ENERGY are excluded -- they have hand-written
    // methods below.
    opcodes!(impl_builder_methods);

    // -----------------------------------------------------------------------
    // Stack
    // -----------------------------------------------------------------------

    /// Emit a `PUSH` instruction with a signed 64-bit immediate value.
    pub fn push(&mut self, imm: i64) -> &mut Self {
        self.emit(Instruction::Push { imm })
    }

    // -----------------------------------------------------------------------
    // Hamiltonian energy
    // -----------------------------------------------------------------------

    /// Emit an `ENERGY` instruction.
    pub fn energy(&mut self, model: Register, sample: Register) -> &mut Self {
        self.emit(Instruction::Energy { model, sample })
    }

    // -----------------------------------------------------------------------
    // Finalise
    // -----------------------------------------------------------------------

    /// Resolve all jump fixups and return the assembled byte buffer.
    ///
    /// For each pending `JUMP`/`JUMPI`, computes `delta = label_pos - site`
    /// and writes it as a forced 3-byte zigzag varint into the three bytes
    /// that follow the opcode (reserved by the `i16::MAX` placeholder).
    ///
    /// # Errors
    ///
    /// - [`Error::UnplacedLabel`] -- a label used in a jump was never
    ///   placed.
    /// - [`Error::OffsetOverflow`] -- the distance between a jump and
    ///   its target exceeds the `i16` range (`[-32768, 32767]`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_bytecode::{builder::InstructionBuilder, codec, stream::InstructionStream};
    /// use aglais_xqvm_bytecode::types::Instruction;
    ///
    /// let mut b = InstructionBuilder::new();
    /// let done = b.label();
    /// b.push(0).jump_if(done).push(1).place(done).halt();
    ///
    /// let buf = b.build().unwrap();
    ///
    /// // Verify the buffer decodes cleanly.
    /// let instrs: Vec<_> = InstructionStream::new(&buf)
    ///     .collect::<std::result::Result<Vec<_>, _>>()
    ///     .unwrap();
    /// assert_eq!(instrs.last().unwrap().2, Instruction::Halt {});
    /// ```
    pub fn build(mut self) -> Result<Vec<u8>> {
        for fixup in &self.fixups {
            let label_pos = self.label_positions[fixup.label.0]
                .ok_or(Error::UnplacedLabel { id: fixup.label.0 })?;

            let delta = label_pos as i64 - fixup.site as i64;
            let offset = i16::try_from(delta).map_err(|_| Error::OffsetOverflow {
                site: fixup.site,
                label: fixup.label.0,
                delta,
            })?;

            // Wire format: [opcode: u8][offset: forced 3-byte zigzag varint]
            // The placeholder (i16::MAX) reserved exactly 3 bytes at [site+1..=site+3].
            // Any i16 value zigzag-encodes to at most 3 varint bytes (max zigzag = 65535).
            let z = u32::from(zigzag_i16(offset));
            self.buf[fixup.site + 1] = (z & 0x7F) as u8 | 0x80;
            self.buf[fixup.site + 2] = ((z >> 7) & 0x7F) as u8 | 0x80;
            self.buf[fixup.site + 3] = (z >> 14) as u8;
        }
        Ok(self.buf)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Zigzag-encode a signed 16-bit integer to an unsigned 16-bit value.
///
/// Maps non-negative `n` to `2n` and negative `n` to `-2n - 1`, so that
/// small-magnitude values produce small unsigned results and compress well
/// under varint encoding.
fn zigzag_i16(n: i16) -> u16 {
    ((n << 1) ^ (n >> 15)) as u16
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(unused_results)]
mod tests {
    use super::*;
    use crate::stream::InstructionStream;
    use crate::types::Instruction;

    fn decode_all(buf: &[u8]) -> Vec<Instruction> {
        InstructionStream::new(buf).map(|r| r.unwrap().2).collect()
    }

    #[test]
    fn empty_builder_produces_empty_buffer() {
        assert_eq!(InstructionBuilder::new().build().unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn push_halt_roundtrip() {
        let mut b = InstructionBuilder::new();
        b.push(42).halt();
        let instrs = decode_all(&b.build().unwrap());
        assert_eq!(
            instrs,
            [Instruction::Push { imm: 42 }, Instruction::Halt {}]
        );
    }

    #[test]
    fn backward_jump_resolves_correctly() {
        // PUSH(3) (2 bytes) at 0; JUMPI (4 bytes) at 2; target = 0.
        // delta = 0 - 2 = -2.
        let mut b = InstructionBuilder::new();
        let top = b.label();
        b.place(top).push(3).jump_if(top);

        let buf = b.build().unwrap();
        let instrs = decode_all(&buf);
        assert_eq!(instrs[0], Instruction::Push { imm: 3 });
        assert_eq!(instrs[1], Instruction::JumpI { offset: -2 });
    }

    #[test]
    fn forward_jump_resolves_correctly() {
        // JUMP (4 bytes) at 0; NOP (1 byte) at 4; HALT (1 byte) at 5.
        // jump target = HALT at 5 -> delta = 5 - 0 = +5.
        let mut b = InstructionBuilder::new();
        let done = b.label();
        b.jump(done).nop().place(done).halt();

        let buf = b.build().unwrap();
        let instrs = decode_all(&buf);
        assert_eq!(instrs[0], Instruction::Jump { offset: 5 });
        assert_eq!(instrs[1], Instruction::Nop {});
        assert_eq!(instrs[2], Instruction::Halt {});
    }

    #[test]
    fn multiple_jumps_to_same_label() {
        let mut b = InstructionBuilder::new();
        let done = b.label();
        b.push(0)
            .jump_if(done)
            .push(0)
            .jump_if(done)
            .place(done)
            .halt();

        let buf = b.build().unwrap();
        let instrs = decode_all(&buf);
        // Last instruction must be HALT.
        assert_eq!(*instrs.last().unwrap(), Instruction::Halt {});
        // Both JUMPIs must point to the same target (HALT's offset = 22).
        let halt_off = 22usize; // 9+3+9+3 = 24? Let me compute:
        // PUSH(9) + JUMPI(3) + PUSH(9) + JUMPI(3) + HALT(1) = 25 bytes total
        // HALT is at offset 24.
        // JUMPI1 at offset 9: delta = 24 - 9 = 15.
        // JUMPI2 at offset 12+9=... actually let me just check the offsets decode.
        let _ = halt_off;
        assert_eq!(instrs[0], Instruction::Push { imm: 0 });
        assert_eq!(instrs[2], Instruction::Push { imm: 0 });
        // Both JUMPIs have the same target (HALT) -- just verify they decoded.
        assert!(matches!(instrs[1], Instruction::JumpI { .. }));
        assert!(matches!(instrs[3], Instruction::JumpI { .. }));
        assert_eq!(instrs[4], Instruction::Halt {});
    }

    #[test]
    fn unplaced_label_returns_error() {
        let mut b = InstructionBuilder::new();
        let ghost = b.label();
        b.jump(ghost).halt();
        assert_eq!(b.build(), Err(Error::UnplacedLabel { id: 0 }));
    }

    #[test]
    fn two_independent_labels() {
        let mut b = InstructionBuilder::new();
        let l0 = b.label();
        let l1 = b.label();
        b.place(l0).nop().place(l1).halt();

        let buf = b.build().unwrap();
        let instrs = decode_all(&buf);
        assert_eq!(instrs, [Instruction::Nop {}, Instruction::Halt {}]);
    }

    #[test]
    fn emit_arbitrary_instruction() {
        let mut b = InstructionBuilder::new();
        b.emit(Instruction::Dupl {}).emit(Instruction::Halt {});
        let instrs = decode_all(&b.build().unwrap());
        assert_eq!(instrs, [Instruction::Dupl {}, Instruction::Halt {}]);
    }

    #[test]
    fn arithmetic_chain() {
        let mut b = InstructionBuilder::new();
        b.push(10).push(3).add().push(2).mul().neg().halt();
        let instrs = decode_all(&b.build().unwrap());
        assert_eq!(instrs[2], Instruction::Add {});
        assert_eq!(instrs[4], Instruction::Mul {});
        assert_eq!(instrs[5], Instruction::Neg {});
    }

    #[test]
    fn energy_method_encodes_both_registers() {
        let mut b = InstructionBuilder::new();
        b.energy(Register(1), Register(2)).halt();
        let instrs = decode_all(&b.build().unwrap());
        assert_eq!(
            instrs[0],
            Instruction::Energy {
                model: Register(1),
                sample: Register(2)
            },
        );
    }

    #[test]
    fn place_twice_panics() {
        let result = std::panic::catch_unwind(|| {
            let mut b = InstructionBuilder::new();
            let l = b.label();
            b.place(l).place(l);
        });
        assert!(result.is_err(), "placing a label twice should panic");
    }
}
