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
//! use aglais_xqvm_bytecode::InstructionBuilder;
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
//! let program = b.build().unwrap();
//! assert!(!program.code().is_empty());
//! ```
//!
//! Forward references work equally well:
//!
//! ```rust
//! use aglais_xqvm_bytecode::InstructionBuilder;
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
//! let program = b.build().unwrap();
//! assert!(!program.code().is_empty());
//! ```

use thiserror::Error;

use crate::codec;
use crate::program::Program;
use crate::types::{Instruction, Opcode, Register};

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

type Result<T> = std::result::Result<T, Error>;

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
    opcode: Opcode,
    label: LabelId,
}

/// Fluent XQVM bytecode assembler.
///
/// Create a builder with [`new`](Self::new), allocate labels with
/// [`label`](Self::label), emit instructions through the named methods or
/// [`emit`](Self::emit), anchor labels with [`place`](Self::place), and
/// finalise the buffer with [`build`](Self::build).
///
/// Use [`push`](Self::push) to emit the smallest `PUSHC_N` instruction that
/// faithfully represents the given `i64` value.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_bytecode::InstructionBuilder;
/// use aglais_xqvm_bytecode::{Instruction, Register};
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
/// let program = b.build().unwrap();
/// assert!(!program.code().is_empty());
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
// the opcode table.  Entries with `offset`, `model`, or `val` fields are
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

    // Skip ENERGY -- `{model: ...}`
    ( ($code:literal, $variant:ident, $mnem:literal, $doc:literal,
       {model: $($rest_f:tt)*}), $($rest:tt)* ) => {
        impl_builder_methods!($($rest)*);
    };

    // Skip PUSHC_1..PUSHC_8 -- `{val: ...}` -- dedicated push() handles them.
    ( ($code:literal, $variant:ident, $mnem:literal, $doc:literal,
       {val: $($rest_f:tt)*}), $($rest:tt)* ) => {
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
    /// use aglais_xqvm_bytecode::InstructionBuilder;
    ///
    /// let mut b = InstructionBuilder::new();
    /// let top = b.label();
    /// b.place(top).nop().jump(top);
    /// let program = b.build().unwrap();
    /// assert!(!program.code().is_empty());
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
        let slot = self
            .label_positions
            .get_mut(label.0)
            .unwrap_or_else(|| panic!("label {} not created by this builder", label.0));
        assert!(
            slot.is_none(),
            "label {id} placed more than once",
            id = label.0,
        );
        *slot = Some(self.buf.len());
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
        self.emit_with_fixup(Instruction::Jump { offset: i16::MAX }, label)
    }

    /// Emit a `JUMPI` instruction targeting `label`.
    ///
    /// Jumps if the top of the stack is non-zero. The actual `i16` offset is
    /// filled in during [`build`](Self::build).
    pub fn jump_if(&mut self, label: LabelId) -> &mut Self {
        self.emit_with_fixup(Instruction::JumpI { offset: i16::MAX }, label)
    }

    fn emit_with_fixup(&mut self, instr: Instruction, label: LabelId) -> &mut Self {
        let site = self.buf.len();
        let enc = codec::encode(&instr);
        self.buf.extend_from_slice(&enc);
        self.fixups.push(Fixup {
            site,
            opcode: instr.opcode(),
            label,
        });
        self
    }

    // Generated from the opcode table: no-arg and single-register methods.
    // JUMP, JUMPI, ENERGY, and PUSHC_1..PUSHC_8 are excluded -- they have
    // hand-written methods.
    opcodes!(impl_builder_methods);

    // -----------------------------------------------------------------------
    // Stack
    // -----------------------------------------------------------------------

    /// Emit the smallest `PUSHC_N` instruction that faithfully represents `val`.
    ///
    /// * `val == 0`  -- [`PushC0`](Instruction::PushC0) (1 byte)
    /// * fits in i8  -- [`PushC1`](Instruction::PushC1) (2 bytes)
    /// * fits in i16 -- [`PushC2`](Instruction::PushC2) (3 bytes)
    /// * fits in i24 -- [`PushC3`](Instruction::PushC3) (4 bytes)
    /// * fits in i32 -- [`PushC4`](Instruction::PushC4) (5 bytes)
    /// * fits in i40 -- [`PushC5`](Instruction::PushC5) (6 bytes)
    /// * fits in i48 -- [`PushC6`](Instruction::PushC6) (7 bytes)
    /// * fits in i56 -- [`PushC7`](Instruction::PushC7) (8 bytes)
    /// * any i64     -- [`PushC8`](Instruction::PushC8) (9 bytes)
    pub fn push(&mut self, val: i64) -> &mut Self {
        self.emit(minimal_pushc(val))
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

    /// Resolve all jump fixups and return the assembled [`Program`].
    ///
    /// For each pending `JUMP`/`JUMPI`, computes `delta = label_pos - site`
    /// and re-encodes the instruction with the resolved `i16` offset in
    /// place of the placeholder. The returned [`Program`] contains the final
    /// instruction bytes.
    ///
    /// # Errors
    ///
    /// - [`Error::UnplacedLabel`] -- a label used in a jump was never placed.
    /// - [`Error::OffsetOverflow`] -- the distance between a jump and its
    ///   target exceeds the `i16` range (`[-32768, 32767]`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_bytecode::{InstructionBuilder, InstructionStream};
    /// use aglais_xqvm_bytecode::Instruction;
    ///
    /// let mut b = InstructionBuilder::new();
    /// let done = b.label();
    /// b.push(0).jump_if(done).push(1).place(done).halt();
    ///
    /// let program = b.build().unwrap();
    ///
    /// // Verify the instruction bytes decode cleanly.
    /// let instrs: Vec<_> = InstructionStream::new(program.code())
    ///     .collect::<std::result::Result<Vec<_>, _>>()
    ///     .unwrap();
    /// assert_eq!(instrs.last().unwrap().2, Instruction::Halt {});
    /// ```
    pub fn build(mut self) -> Result<Program> {
        for fixup in &self.fixups {
            let label_pos = self
                .label_positions
                .get(fixup.label.0)
                .copied()
                .flatten()
                .ok_or(Error::UnplacedLabel { id: fixup.label.0 })?;

            let delta = label_pos as i64 - fixup.site as i64;
            let offset = i16::try_from(delta).map_err(|_| Error::OffsetOverflow {
                site: fixup.site,
                label: fixup.label.0,
                delta,
            })?;

            let instr = match fixup.opcode {
                Opcode::Jump => Instruction::Jump { offset },
                Opcode::JumpI => Instruction::JumpI { offset },
                _ => unreachable!("fixups are emitted only for jumps"),
            };
            let encoded = codec::encode(&instr);
            let end = fixup.site + encoded.len();
            self.buf
                .get_mut(fixup.site..end)
                .unwrap_or_else(|| panic!("fixup site {:#06X} out of buffer bounds", fixup.site))
                .copy_from_slice(&encoded);
        }
        Ok(Program::new(self.buf))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the smallest `PUSHC_N` instruction that faithfully represents `val`.
fn minimal_pushc(val: i64) -> Instruction {
    if val == 0 {
        return Instruction::PushC0 {};
    }
    let be = val.to_be_bytes();
    for n in 1usize..=7 {
        let bits = (n * 8) as u32;
        let shift = 64 - bits;
        if (val << shift) >> shift == val {
            return match n {
                1 => Instruction::PushC1 { val: [be[7]] },
                2 => Instruction::PushC2 { val: [be[6], be[7]] },
                3 => Instruction::PushC3 { val: [be[5], be[6], be[7]] },
                4 => Instruction::PushC4 { val: [be[4], be[5], be[6], be[7]] },
                5 => Instruction::PushC5 { val: [be[3], be[4], be[5], be[6], be[7]] },
                6 => Instruction::PushC6 { val: [be[2], be[3], be[4], be[5], be[6], be[7]] },
                7 => Instruction::PushC7 {
                    val: [be[1], be[2], be[3], be[4], be[5], be[6], be[7]],
                },
                _ => unreachable!(),
            };
        }
    }
    Instruction::PushC8 { val: be }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(unused_results, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::stream::InstructionStream;
    use crate::types::Instruction;

    fn decode_all(buf: &[u8]) -> Vec<Instruction> {
        InstructionStream::new(buf).map(|r| r.unwrap().2).collect()
    }

    #[test]
    fn empty_builder_produces_empty_code() {
        let program = InstructionBuilder::new().build().unwrap();
        assert!(program.code().is_empty());
    }

    #[test]
    fn push_zero_emits_pushc0() {
        let mut b = InstructionBuilder::new();
        b.push(0).halt();
        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(instrs[0], Instruction::PushC0 {});
    }

    #[test]
    fn push_i8_emits_pushc1() {
        let mut b = InstructionBuilder::new();
        b.push(42).halt();
        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(instrs[0], Instruction::PushC1 { val: [42] });
    }

    #[test]
    fn push_minus_one_emits_pushc1() {
        let mut b = InstructionBuilder::new();
        b.push(-1).halt();
        let instrs = decode_all(b.build().unwrap().code());
        // -1 as i8 = 0xFF
        assert_eq!(instrs[0], Instruction::PushC1 { val: [0xFF] });
    }

    #[test]
    fn push_i8_max_uses_pushc1() {
        let mut b = InstructionBuilder::new();
        b.push(127).halt();
        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(instrs[0], Instruction::PushC1 { val: [0x7F] });
    }

    #[test]
    fn push_i8_max_plus_one_uses_pushc2() {
        let mut b = InstructionBuilder::new();
        b.push(128).halt();
        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(instrs[0], Instruction::PushC2 { val: [0x00, 0x80] });
    }

    #[test]
    fn push_i64_max_uses_pushc8() {
        let mut b = InstructionBuilder::new();
        b.push(i64::MAX).halt();
        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(instrs[0], Instruction::PushC8 { val: i64::MAX.to_be_bytes() });
    }

    #[test]
    fn push_i64_min_uses_pushc8() {
        let mut b = InstructionBuilder::new();
        b.push(i64::MIN).halt();
        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(instrs[0], Instruction::PushC8 { val: i64::MIN.to_be_bytes() });
    }

    #[test]
    fn backward_jump_resolves_correctly() {
        // PushC0 (1 byte) at 0; JUMPI (3 bytes) at 1; target = 0.
        // delta = 0 - 1 = -1.
        let mut b = InstructionBuilder::new();
        let top = b.label();
        b.place(top).push(0).jump_if(top);

        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(instrs[0], Instruction::PushC0 {});
        assert_eq!(instrs[1], Instruction::JumpI { offset: -1 });
    }

    #[test]
    fn forward_jump_resolves_correctly() {
        // JUMP (3 bytes) at 0; NOP (1 byte) at 3; HALT (1 byte) at 4.
        // jump target = HALT at 4 -> delta = 4 - 0 = +4.
        let mut b = InstructionBuilder::new();
        let done = b.label();
        b.jump(done).nop().place(done).halt();

        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(instrs[0], Instruction::Jump { offset: 4 });
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

        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(*instrs.last().unwrap(), Instruction::Halt {});
        assert_eq!(instrs[0], Instruction::PushC0 {});
        assert_eq!(instrs[2], Instruction::PushC0 {});
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

        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(instrs, [Instruction::Nop {}, Instruction::Halt {}]);
    }

    #[test]
    fn emit_arbitrary_instruction() {
        let mut b = InstructionBuilder::new();
        b.emit(Instruction::Dupl {}).emit(Instruction::Halt {});
        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(instrs, [Instruction::Dupl {}, Instruction::Halt {}]);
    }

    #[test]
    fn arithmetic_chain() {
        let mut b = InstructionBuilder::new();
        b.push(10).push(3).add().push(2).mul().neg().halt();
        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(instrs[2], Instruction::Add {});
        assert_eq!(instrs[4], Instruction::Mul {});
        assert_eq!(instrs[5], Instruction::Neg {});
    }

    #[test]
    fn energy_method_encodes_both_registers() {
        let mut b = InstructionBuilder::new();
        b.energy(Register(1), Register(2)).halt();
        let instrs = decode_all(b.build().unwrap().code());
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
