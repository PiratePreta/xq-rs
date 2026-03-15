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
//! let program = b.build().unwrap();
//! assert!(!program.code().is_empty());
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
//! let program = b.build().unwrap();
//! assert!(!program.code().is_empty());
//! ```

use thiserror::Error;

use crate::codec;
use crate::pool::{ConstantPool, PoolOverflow};
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

    /// More than 65535 distinct `i64` constants were interned via
    /// [`InstructionBuilder::push_const`].
    #[error("constant pool overflow: more than 65535 distinct i64 constants")]
    PoolOverflow,
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
/// Use [`push_const`](Self::push_const) to intern an `i64` constant into the
/// pool and emit a `PUSHC` instruction. The resulting [`Program`] carries both
/// the pool and the instruction bytes.
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
/// let program = b.build().unwrap();
/// assert!(!program.code().is_empty());
/// ```
#[derive(Debug, Default)]
pub struct InstructionBuilder {
    buf: Vec<u8>,
    /// `None` means allocated but not yet placed.
    label_positions: Vec<Option<usize>>,
    fixups: Vec<Fixup>,
    pool: ConstantPool,
    /// Deferred error: set when `push_const` encounters a pool overflow.
    pool_overflow: bool,
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

    // Skip PUSHC -- `{idx: ...}` (has a dedicated push_const method)
    ( ($code:literal, $variant:ident, $mnem:literal, $doc:literal,
       {idx: $($rest_f:tt)*}), $($rest:tt)* ) => {
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
    // JUMP, JUMPI, PUSH, and ENERGY are excluded -- they have hand-written
    // methods below.
    opcodes!(impl_builder_methods);

    // -----------------------------------------------------------------------
    // Stack
    // -----------------------------------------------------------------------

    /// Emit a `PUSH` or `PUSHC` instruction for the given value.
    ///
    /// If `imm` fits in `i16` (i.e. `-32768..=32767`), the compact
    /// `PUSH imm` form is used. For larger values the constant is interned in
    /// the pool via [`push_const`](Self::push_const) and a `PUSHC` instruction
    /// is emitted instead.
    pub fn push(&mut self, imm: i64) -> &mut Self {
        if let Ok(small) = i16::try_from(imm) {
            self.emit(Instruction::Push { imm: small })
        } else {
            self.push_const(imm)
        }
    }

    /// Intern `imm` in the constant pool and emit a `PUSHC` instruction.
    ///
    /// If `imm` is already in the pool, the existing index is reused and no
    /// new pool entry is added. If the pool is full (more than 65535 distinct
    /// constants), the overflow is recorded and [`build`](Self::build) will
    /// return [`Error::PoolOverflow`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_bytecode::builder::InstructionBuilder;
    /// use aglais_xqvm_bytecode::types::Instruction;
    ///
    /// let mut b = InstructionBuilder::new();
    /// b.push_const(42).push_const(42).halt(); // two refs, one pool entry
    ///
    /// let program = b.build().unwrap();
    /// assert_eq!(program.pool().len(), 1);
    /// assert_eq!(program.pool().get(0), Some(42));
    /// ```
    pub fn push_const(&mut self, imm: i64) -> &mut Self {
        match self.pool.intern(imm) {
            Ok(idx) => {
                self.emit(Instruction::PushC { idx });
            }
            Err(PoolOverflow) => {
                // Record overflow; emit a placeholder so the buffer length
                // stays consistent for subsequent offset computations.
                if !self.pool_overflow {
                    self.pool_overflow = true;
                }
                self.emit(Instruction::PushC { idx: 0 });
            }
        }
        self
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
    /// place of the placeholder. The returned [`Program`] bundles the
    /// constant pool (populated by any [`push_const`](Self::push_const)
    /// calls) with the final instruction bytes.
    ///
    /// # Errors
    ///
    /// - [`Error::PoolOverflow`] -- more than 65535 distinct constants were
    ///   passed to [`push_const`](Self::push_const).
    /// - [`Error::UnplacedLabel`] -- a label used in a jump was never
    ///   placed.
    /// - [`Error::OffsetOverflow`] -- the distance between a jump and
    ///   its target exceeds the `i16` range (`[-32768, 32767]`).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use aglais_xqvm_bytecode::{builder::InstructionBuilder, stream::InstructionStream};
    /// use aglais_xqvm_bytecode::types::Instruction;
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
        if self.pool_overflow {
            return Err(Error::PoolOverflow);
        }
        for fixup in &self.fixups {
            let label_pos = self.label_positions[fixup.label.0]
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
            self.buf[fixup.site..fixup.site + encoded.len()].copy_from_slice(&encoded);
        }
        Ok(Program::new(self.pool, self.buf))
    }
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

    use crate::pool::ConstantPool;
    use crate::program::Program;

    fn decode_all(buf: &[u8]) -> Vec<Instruction> {
        InstructionStream::new(buf).map(|r| r.unwrap().2).collect()
    }

    #[test]
    fn empty_builder_produces_empty_buffer() {
        let program = InstructionBuilder::new().build().unwrap();
        assert_eq!(program, Program::new(ConstantPool::new(), vec![]));
    }

    #[test]
    fn push_halt_roundtrip() {
        let mut b = InstructionBuilder::new();
        b.push(42).halt();
        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(
            instrs,
            [Instruction::Push { imm: 42 }, Instruction::Halt {}]
        );
    }

    #[test]
    fn backward_jump_resolves_correctly() {
        // PUSH(3) (3 bytes) at 0; JUMPI (3 bytes) at 3; target = 0.
        // delta = 0 - 3 = -3.
        let mut b = InstructionBuilder::new();
        let top = b.label();
        b.place(top).push(3).jump_if(top);

        let instrs = decode_all(b.build().unwrap().code());
        assert_eq!(instrs[0], Instruction::Push { imm: 3 });
        assert_eq!(instrs[1], Instruction::JumpI { offset: -3 });
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
        // Last instruction must be HALT.
        assert_eq!(*instrs.last().unwrap(), Instruction::Halt {});
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
    fn push_const_interns_and_emits_pushc() {
        let mut b = InstructionBuilder::new();
        b.push_const(42).halt();

        let program = b.build().unwrap();
        assert_eq!(program.pool().len(), 1);
        assert_eq!(program.pool().get(0), Some(42));
        let instrs = decode_all(program.code());
        assert_eq!(instrs[0], Instruction::PushC { idx: 0 });
        assert_eq!(instrs[1], Instruction::Halt {});
    }

    #[test]
    fn push_const_deduplicates_same_value() {
        let mut b = InstructionBuilder::new();
        b.push_const(99).push_const(99).halt();

        let program = b.build().unwrap();
        assert_eq!(
            program.pool().len(),
            1,
            "duplicate constant not deduplicated"
        );
        let instrs = decode_all(program.code());
        assert_eq!(instrs[0], Instruction::PushC { idx: 0 });
        assert_eq!(instrs[1], Instruction::PushC { idx: 0 });
    }

    #[test]
    fn push_const_multiple_distinct_values() {
        let mut b = InstructionBuilder::new();
        b.push_const(1).push_const(2).push_const(3).halt();

        let program = b.build().unwrap();
        assert_eq!(program.pool().len(), 3);
        let instrs = decode_all(program.code());
        assert_eq!(instrs[0], Instruction::PushC { idx: 0 });
        assert_eq!(instrs[1], Instruction::PushC { idx: 1 });
        assert_eq!(instrs[2], Instruction::PushC { idx: 2 });
    }

    #[test]
    fn build_returns_empty_pool_when_no_push_const() {
        let mut b = InstructionBuilder::new();
        b.push(5).halt();
        let program = b.build().unwrap();
        assert!(program.pool().is_empty());
    }

    #[test]
    fn push_large_value_promotes_to_pushc() {
        // 100_000 does not fit in i16 -- push() should intern it and emit PUSHC.
        let mut b = InstructionBuilder::new();
        b.push(100_000i64).halt();
        let program = b.build().unwrap();
        assert_eq!(program.pool().len(), 1);
        assert_eq!(program.pool().get(0), Some(100_000i64));
        let instrs = decode_all(program.code());
        assert_eq!(instrs[0], Instruction::PushC { idx: 0 });
        assert_eq!(instrs[1], Instruction::Halt {});
    }

    #[test]
    fn push_i16_max_fits_inline() {
        let mut b = InstructionBuilder::new();
        b.push(i64::from(i16::MAX)).halt();
        let program = b.build().unwrap();
        assert!(
            program.pool().is_empty(),
            "i16::MAX should not go through pool"
        );
        let instrs = decode_all(program.code());
        assert_eq!(instrs[0], Instruction::Push { imm: i16::MAX });
    }

    #[test]
    fn push_i16_max_plus_one_promotes_to_pushc() {
        let val = i64::from(i16::MAX) + 1;
        let mut b = InstructionBuilder::new();
        b.push(val).halt();
        let program = b.build().unwrap();
        assert_eq!(program.pool().get(0), Some(val));
        let instrs = decode_all(program.code());
        assert!(matches!(instrs[0], Instruction::PushC { .. }));
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
