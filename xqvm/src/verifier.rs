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

//! Pre-execution bytecode verifier for XQVM programs.
//!
//! Verification is structured as composable [`Phase`]s. Each phase performs
//! one focused check over a [`Program`] and maps failures to [`VerifierError`]
//! via its `type Error: Into<VerifierError>`. A [`Verifier`] runs phases
//! sequentially, stopping at the first failure.
//!
//! The low-level `scan` function is the shared kernel: it makes one linear
//! pass over the instruction bytes, builds the [`JumpTable`], and returns the
//! first Phase 1 violation. [`Program::new`] calls it to obtain the jump
//! table (ignoring any error); [`Verifier::default`] calls it once per
//! invocation rather than running three separate stream passes.
//!
//! # Phases
//!
//! | Phase | What it checks | Errors emitted |
//! |---|---|---|
//! | [`StructuralPhase`] | Truncated bytes, unknown opcodes | [`VerifierError::TruncatedInstruction`], [`VerifierError::BadOpcode`] |
//! | [`JumpTargetPhase`] | Jump label ≥ target count | [`VerifierError::UndefinedJumpTarget`] |
//! | [`LoopNestingPhase`] | Loop open/close balance, loop-context reads | [`VerifierError::NoActiveLoop`], [`VerifierError::UnmatchedLoop`] |
//! | [`RegisterTypePhase`] | Register read-before-write; type-state mismatches | [`VerifierError::ReadUnsetRegister`], [`VerifierError::RegisterTypeMismatch`] |
//!
//! Phase 1 (structural, jump, loop) runs as a single combined pass via
//! `scan`. Phase 2 ([`RegisterTypePhase`]) performs a forward type-state
//! analysis over `[RegType; 256]` -- no control-flow graph is built; the
//! scan is linear and conservative.
//!
//! # Quick start
//!
//! ```rust
//! use xqvm::{verifier, InstructionBuilder};
//!
//! let mut b = InstructionBuilder::new();
//! b.emit_push(1).emit_push(2).emit_add().emit_halt();
//! let program = b.build().unwrap();
//! assert!(verifier::verify(&program).is_ok());
//! ```
//!
//! # Custom composition
//!
//! ```rust
//! use xqvm::verifier::{Phase, Verifier, JumpTargetPhase, LoopNestingPhase};
//! use xqvm::InstructionBuilder;
//!
//! let mut b = InstructionBuilder::new();
//! b.emit_halt();
//! let program = b.build().unwrap();
//!
//! // Run only the jump and loop checks, skip the structural pass.
//! let result = Verifier::new()
//!     .with_phase(JumpTargetPhase)
//!     .with_phase(LoopNestingPhase)
//!     .run(&program);
//! assert!(result.is_ok());
//! ```

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, vec::Vec};

use thiserror::Error;

use crate::bytecode::error::StreamError;
use crate::bytecode::{Instruction, InstructionStream, JumpTable, Program, StackEffect};

// ---------------------------------------------------------------------------
// VerifierError
// ---------------------------------------------------------------------------

/// Errors produced by the pre-execution bytecode verifier.
///
/// Each variant carries the byte offset of the offending instruction so that
/// diagnostics can highlight the exact location in the bytecode stream.
#[derive(Debug, Error)]
#[expect(
    missing_docs,
    reason = "error variants are documented via their #[error(...)] display strings"
)]
pub enum VerifierError {
    /// A jump instruction references a `TARGET` label id that does not exist
    /// in the program's jump table.
    #[error(
        "jump at byte {offset:#06x} references undefined target label {label} \
         (program has {target_count} targets)"
    )]
    UndefinedJumpTarget {
        offset: usize,
        label: u16,
        target_count: usize,
    },

    /// `NEXT`, `LVAL`, or `LIDX` was reached with no `RANGE`/`ITER` active.
    #[error("loop instruction at byte {offset:#06x} executed outside any active loop")]
    NoActiveLoop { offset: usize },

    /// At least one `RANGE` or `ITER` was opened without a matching `NEXT`
    /// before the end of the program. `offset` is the byte position of the
    /// outermost unmatched loop opener.
    #[error(
        "unmatched loop: RANGE/ITER at byte {offset:#06x} has no corresponding NEXT \
         ({depth} loop(s) still open at end of program)"
    )]
    UnmatchedLoop { offset: usize, depth: usize },

    /// The instruction stream hit a truncated operand sequence.
    #[error("truncated instruction at byte {offset:#06x}")]
    TruncatedInstruction { offset: usize },

    /// The instruction stream encountered an unrecognized opcode byte.
    #[error("unknown opcode {byte:#04x} at byte {offset:#06x}")]
    BadOpcode { offset: usize, byte: u8 },

    /// An instruction tried to read a register that has never been written.
    #[error("register r{reg} read at byte {offset:#06x} before being written")]
    ReadUnsetRegister { offset: usize, reg: u8 },

    /// An instruction found a register holding the wrong type.
    #[error("register r{reg} at byte {offset:#06x}: expected {expected}, got {got}")]
    RegisterTypeMismatch {
        offset: usize,
        reg: u8,
        expected: &'static str,
        got: &'static str,
    },

    /// A stack-consuming instruction was reached with insufficient items on the stack.
    #[error("stack underflow at byte {offset:#06x}")]
    StackUnderflow { offset: usize },

    /// The stack depth may exceed the VM limit of 8192 at this instruction.
    #[error("potential stack overflow at byte {offset:#06x} (depth {depth})")]
    StackOverflowRisk { offset: usize, depth: usize },

    /// A loop body's net stack effect is non-zero: the depth at `NEXT` differs
    /// from the depth at the matching `RANGE`/`ITER` opener.
    #[error(
        "loop starting at byte {opener:#06x} has non-zero stack effect \
         (entry depth {entry}, exit depth {exit})"
    )]
    LoopStackImbalance {
        opener: usize,
        entry: usize,
        exit: usize,
    },
}

impl VerifierError {
    /// A stable string tag for each variant used by the conformance harness
    /// to match expected errors without coupling to the `Display` format.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::UndefinedJumpTarget { .. } => "UndefinedJumpTarget",
            Self::NoActiveLoop { .. } => "NoActiveLoop",
            Self::UnmatchedLoop { .. } => "UnmatchedLoop",
            Self::TruncatedInstruction { .. } => "TruncatedInstruction",
            Self::BadOpcode { .. } => "BadOpcode",
            Self::ReadUnsetRegister { .. } => "ReadUnsetRegister",
            Self::RegisterTypeMismatch { .. } => "RegisterTypeMismatch",
            Self::StackUnderflow { .. } => "StackUnderflow",
            Self::StackOverflowRisk { .. } => "StackOverflowRisk",
            Self::LoopStackImbalance { .. } => "LoopStackImbalance",
        }
    }
}

// ---------------------------------------------------------------------------
// Phase trait
// ---------------------------------------------------------------------------

/// A single composable verification pass over a [`Program`].
///
/// `type Error` is the phase-specific failure type. It must implement
/// `Into<VerifierError>` so it can be collected by a [`Verifier`].
///
/// # Implementing a phase
///
/// ```rust
/// use xqvm::verifier::{Phase, VerifierError};
/// use xqvm::bytecode::Program;
///
/// pub struct NopPhase;
///
/// impl Phase for NopPhase {
///     type Error = VerifierError;
///     fn run(&self, _program: &Program) -> Result<(), VerifierError> {
///         Ok(())
///     }
/// }
/// ```
pub trait Phase {
    /// The error this phase can produce. Must be convertible to [`VerifierError`].
    type Error: Into<VerifierError>;

    /// Run this phase over `program`.
    ///
    /// # Errors
    ///
    /// Returns a phase-specific error on the first violation detected.
    fn run(&self, program: &Program) -> Result<(), Self::Error>;
}

// ---------------------------------------------------------------------------
// Verifier
// ---------------------------------------------------------------------------

/// A sequenced collection of [`Phase`]s that run over a [`Program`].
///
/// Build a verifier with the fluent [`with_phase`](Self::with_phase) method.
/// [`Verifier::default`] creates the standard verifier with all
/// built-in phases in the recommended order.
///
/// Phases run sequentially; the first failure stops the chain (fail-fast).
///
/// # Examples
///
/// ```rust
/// use xqvm::verifier::{Verifier, StructuralPhase, JumpTargetPhase};
/// use xqvm::InstructionBuilder;
///
/// let mut b = InstructionBuilder::new();
/// b.emit_halt();
/// let program = b.build().unwrap();
///
/// let result = Verifier::new()
///     .with_phase(StructuralPhase)
///     .with_phase(JumpTargetPhase)
///     .run(&program);
/// assert!(result.is_ok());
/// ```
pub struct Verifier {
    phases: Vec<PhaseBox>,
}

/// Type-erased, heap-allocated phase closure.
type PhaseBox = Box<dyn Fn(&Program) -> Result<(), VerifierError>>;

impl Verifier {
    /// Create an empty verifier with no phases.
    pub fn new() -> Self {
        Self { phases: Vec::new() }
    }

    /// Append a phase to this verifier.
    ///
    /// The phase is type-erased: its concrete `Phase::Error` is converted to
    /// [`VerifierError`] via `Into` at the point of insertion.
    #[must_use]
    pub fn with_phase<P>(mut self, phase: P) -> Self
    where
        P: Phase + 'static,
    {
        self.phases
            .push(Box::new(move |prog| phase.run(prog).map_err(Into::into)));
        self
    }

    /// Run all phases in insertion order, returning the first error.
    ///
    /// # Errors
    ///
    /// Returns the first [`VerifierError`] produced by any phase.
    pub fn run(&self, program: &Program) -> Result<(), VerifierError> {
        for phase in &self.phases {
            phase(program)?;
        }
        Ok(())
    }
}

impl Default for Verifier {
    /// Creates the standard Phase 1 + Phase 2 + Phase 3 verifier.
    ///
    /// Runs in order:
    /// 1. `CombinedPhase` -- structural, jump-target, and loop-nesting checks.
    /// 2. [`RegisterTypePhase`] -- register read-before-write and type-state checks.
    /// 3. [`StackEffectPhase`] -- stack depth tracking, overflow/underflow detection,
    ///    and loop body net-effect check.
    fn default() -> Self {
        Self::new()
            .with_phase(CombinedPhase)
            .with_phase(RegisterTypePhase)
            .with_phase(StackEffectPhase)
    }
}

// ---------------------------------------------------------------------------
// CombinedPhase (private) — one-pass Phase 1 implementation
// ---------------------------------------------------------------------------

/// Runs the full Phase 1 check set in a single stream pass via [`scan`].
///
/// Used by [`Verifier::default`]; not exposed publicly because callers that
/// need selective phase composition should use the individual named phases.
struct CombinedPhase;

impl Phase for CombinedPhase {
    type Error = VerifierError;

    fn run(&self, program: &Program) -> Result<(), VerifierError> {
        let (_table, err) = scan(program.code());
        err.map_or(Ok(()), Err)
    }
}

// ---------------------------------------------------------------------------
// Shared helper
// ---------------------------------------------------------------------------

/// Convert a [`StreamError`] from sequential stream iteration into a
/// [`VerifierError`]. `SeekOutOfBounds` cannot be triggered by sequential
/// iteration, so it is mapped defensively to `TruncatedInstruction`.
fn stream_err(e: &StreamError) -> VerifierError {
    match e {
        StreamError::TruncatedInstruction { offset } => {
            VerifierError::TruncatedInstruction { offset: *offset }
        }
        StreamError::UnknownOpcode { offset, byte } => VerifierError::BadOpcode {
            offset: *offset,
            byte: *byte,
        },
        StreamError::SeekOutOfBounds { target, .. } => {
            VerifierError::TruncatedInstruction { offset: *target }
        }
    }
}

// ---------------------------------------------------------------------------
// Combined single-pass scan
// ---------------------------------------------------------------------------

/// Build a [`JumpTable`] and detect the first Phase 1 violation in one pass.
///
/// Walks the instruction bytes exactly once, collecting:
/// - byte positions of `TARGET` opcodes (for the jump table),
/// - loop-opener positions (to check nesting balance),
/// - jump-instruction labels (for deferred target-count validation).
///
/// Structural errors (`BadOpcode`, `TruncatedInstruction`) abort the walk
/// immediately. Loop and jump-target errors are recorded after the walk.
///
/// Called by [`Program::new`] to obtain the [`JumpTable`] and by
/// [`Verifier::default`] to check for violations. Both callers discard the
/// half of the result they do not need.
pub(crate) fn scan(code: &[u8]) -> (JumpTable, Option<VerifierError>) {
    let mut targets: Vec<usize> = Vec::new();
    let mut loop_offsets: Vec<usize> = Vec::new();
    let mut jump_refs: Vec<(usize, u16)> = Vec::new();
    let mut first_error: Option<VerifierError> = None;

    let mut stream = InstructionStream::new(code);
    'scan: while let Some(item) = stream.next_instruction() {
        let (pos, _label, instr) = match item {
            Ok(v) => v,
            Err(e) => {
                let _ = first_error.get_or_insert_with(|| stream_err(&e));
                break 'scan;
            }
        };

        match instr {
            Instruction::Target {} => targets.push(pos),
            Instruction::Range {} | Instruction::Iter { .. } => loop_offsets.push(pos),
            Instruction::Next {} if first_error.is_none() && loop_offsets.pop().is_none() => {
                first_error = Some(VerifierError::NoActiveLoop { offset: pos });
            }
            Instruction::Lidx { .. } | Instruction::LVal { .. } if loop_offsets.is_empty() => {
                let _ = first_error.get_or_insert(VerifierError::NoActiveLoop { offset: pos });
            }
            Instruction::Jump1 { label } | Instruction::JumpI1 { label } => {
                jump_refs.push((pos, u16::from(label)));
            }
            Instruction::Jump2 { label } | Instruction::JumpI2 { label } => {
                jump_refs.push((pos, label));
            }
            _ => {}
        }
    }

    // Unmatched loop openers: report the outermost (first-pushed) one.
    if first_error.is_none()
        && let Some(&outermost) = loop_offsets.first()
    {
        first_error = Some(VerifierError::UnmatchedLoop {
            offset: outermost,
            depth: loop_offsets.len(),
        });
    }

    let jump_table = JumpTable::new(targets);

    // Deferred jump-target check: needs the final TARGET count.
    if first_error.is_none() {
        let target_count = jump_table.len();
        for (offset, label) in jump_refs {
            if usize::from(label) >= target_count {
                first_error = Some(VerifierError::UndefinedJumpTarget {
                    offset,
                    label,
                    target_count,
                });
                break;
            }
        }
    }

    (jump_table, first_error)
}

// ---------------------------------------------------------------------------
// StructuralPhase
// ---------------------------------------------------------------------------

/// Checks that every byte in the instruction stream decodes without error.
///
/// Catches [`VerifierError::TruncatedInstruction`] and
/// [`VerifierError::BadOpcode`]. This phase should run first so subsequent
/// phases can safely iterate the stream without re-checking decode errors.
pub struct StructuralPhase;

impl Phase for StructuralPhase {
    type Error = VerifierError;

    fn run(&self, program: &Program) -> Result<(), VerifierError> {
        let mut stream = InstructionStream::new(program.code());
        while let Some(item) = stream.next_instruction() {
            // Decode to surface errors; the decoded instruction is not needed here.
            let _ = item.map_err(|e| stream_err(&e))?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// JumpTargetPhase
// ---------------------------------------------------------------------------

/// Checks that every `JUMP*`/`JUMPI*` instruction references a label id that
/// exists in the program's jump table.
///
/// The jump table is pre-computed at [`Program::new`] time, so each label
/// lookup is O(1).
pub struct JumpTargetPhase;

impl Phase for JumpTargetPhase {
    type Error = VerifierError;

    fn run(&self, program: &Program) -> Result<(), VerifierError> {
        let target_count = program.jump_table().len();
        let mut stream = InstructionStream::new(program.code());

        while let Some(item) = stream.next_instruction() {
            let (pos, _label, instr) = item.map_err(|e| stream_err(&e))?;
            match instr {
                Instruction::Jump1 { label } | Instruction::JumpI1 { label }
                    if usize::from(label) >= target_count =>
                {
                    return Err(VerifierError::UndefinedJumpTarget {
                        offset: pos,
                        label: u16::from(label),
                        target_count,
                    });
                }
                Instruction::Jump2 { label } | Instruction::JumpI2 { label }
                    if usize::from(label) >= target_count =>
                {
                    return Err(VerifierError::UndefinedJumpTarget {
                        offset: pos,
                        label,
                        target_count,
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// LoopNestingPhase
// ---------------------------------------------------------------------------

/// Checks that `RANGE`/`ITER` and `NEXT` are properly balanced, and that
/// `LVAL`/`LIDX` are only used inside an active loop.
///
/// `loop_offsets` tracks the byte position of each open loop opener in
/// nesting order. Using a stack (rather than a counter) lets the error
/// report the outermost unmatched opener's position.
pub struct LoopNestingPhase;

impl Phase for LoopNestingPhase {
    type Error = VerifierError;

    fn run(&self, program: &Program) -> Result<(), VerifierError> {
        let mut loop_offsets: Vec<usize> = Vec::new();
        let mut stream = InstructionStream::new(program.code());

        while let Some(item) = stream.next_instruction() {
            let (pos, _label, instr) = item.map_err(|e| stream_err(&e))?;
            match instr {
                Instruction::Range {} | Instruction::Iter { .. } => {
                    loop_offsets.push(pos);
                }
                Instruction::Next {} => {
                    // `ok_or` converts None (empty stack) into a NoActiveLoop error.
                    let _ = loop_offsets
                        .pop()
                        .ok_or(VerifierError::NoActiveLoop { offset: pos })?;
                }
                Instruction::Lidx { .. } | Instruction::LVal { .. } if loop_offsets.is_empty() => {
                    return Err(VerifierError::NoActiveLoop { offset: pos });
                }
                _ => {}
            }
        }

        // Report the outermost (first-pushed) unmatched loop opener, if any.
        match loop_offsets.first().copied() {
            Some(outermost) => Err(VerifierError::UnmatchedLoop {
                offset: outermost,
                depth: loop_offsets.len(),
            }),
            None => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------
// RegType — static type tag for register contents
// ---------------------------------------------------------------------------

/// Static type tag used during register type-state analysis.
///
/// Mirrors the [`crate::RegVal`] variants without carrying any value, plus an
/// [`Any`](Self::Any) variant for registers whose type is known to be set but
/// cannot be determined statically (e.g. written by `INPUT` or `LVAL`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RegType {
    /// Register has never been written (default state).
    #[default]
    Unset,
    /// Holds an `i64` integer.
    Int,
    /// Holds a `Vec<i64>` integer vector.
    VecInt,
    /// Holds a `Vec<XqmxModel>` model vector.
    VecXqmx,
    /// Holds an `XqmxModel`.
    Model,
    /// Holds an `XqmxSample`.
    Sample,
    /// Set to an unknown type (e.g. after `INPUT` or `LVAL`). Satisfies any
    /// read requirement so the verifier does not emit false positives.
    Any,
}

impl RegType {
    /// Human-readable name used in [`VerifierError`] messages.
    pub fn type_name(self) -> &'static str {
        match self {
            Self::Unset => "unset",
            Self::Int => "int",
            Self::VecInt => "vec<int>",
            Self::VecXqmx => "vec<xqmx>",
            Self::Model => "model",
            Self::Sample => "sample",
            Self::Any => "any",
        }
    }

    /// Whether this type satisfies `req`.
    ///
    /// `Unset` never satisfies any requirement. `Any` always does (it is
    /// non-unset, just of unknown specific type). All other types match
    /// only the requirements they implement.
    fn satisfies(self, req: RegTypeReq) -> bool {
        match self {
            Self::Unset => false,
            Self::Any => true,
            Self::Int => matches!(req, RegTypeReq::NonUnset | RegTypeReq::Int),
            Self::VecInt => {
                matches!(
                    req,
                    RegTypeReq::NonUnset | RegTypeReq::VecInt | RegTypeReq::AnyVec
                )
            }
            Self::VecXqmx => matches!(req, RegTypeReq::NonUnset | RegTypeReq::AnyVec),
            Self::Model => {
                matches!(
                    req,
                    RegTypeReq::NonUnset | RegTypeReq::Model | RegTypeReq::Grid
                )
            }
            Self::Sample => {
                matches!(
                    req,
                    RegTypeReq::NonUnset | RegTypeReq::Sample | RegTypeReq::Grid
                )
            }
        }
    }
}

/// Type requirement at a register read site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegTypeReq {
    /// Any non-`Unset` value (e.g. `OUTPUT`, `LOAD` raw check).
    NonUnset,
    /// Specifically `Int` (e.g. `LOAD`).
    Int,
    /// Specifically `Model`.
    Model,
    /// Specifically `Sample`.
    Sample,
    /// Specifically `VecInt`.
    VecInt,
    /// `VecInt` or `VecXqmx` (e.g. `ITER`, `VECLEN`).
    AnyVec,
    /// `Model` or `Sample` (XQMX grid operations: `RESIZE`, `ROWFIND`, …).
    Grid,
}

impl RegTypeReq {
    fn type_name(self) -> &'static str {
        match self {
            Self::NonUnset => "any",
            Self::Int => "int",
            Self::Model => "model",
            Self::Sample => "sample",
            Self::VecInt => "vec<int>",
            Self::AnyVec => "vec",
            Self::Grid => "model or sample",
        }
    }
}

/// Check that `slot` satisfies `req`; emit the appropriate [`VerifierError`].
fn check_reg(
    offset: usize,
    slot: u8,
    req: RegTypeReq,
    regs: &[RegType; 256],
) -> Result<(), VerifierError> {
    // slot is u8 (0–255) and regs has exactly 256 elements, so get() never returns None.
    debug_assert!(
        usize::from(slot) < regs.len(),
        "slot u8 always fits in [RegType; 256]"
    );
    let current = regs.get(usize::from(slot)).copied().unwrap_or_default();
    if current == RegType::Unset {
        return Err(VerifierError::ReadUnsetRegister { offset, reg: slot });
    }
    if !current.satisfies(req) {
        return Err(VerifierError::RegisterTypeMismatch {
            offset,
            reg: slot,
            expected: req.type_name(),
            got: current.type_name(),
        });
    }
    Ok(())
}

/// Validate register type requirements for all register-reading instructions.
fn check_reads(
    pos: usize,
    instr: &Instruction,
    regs: &[RegType; 256],
) -> Result<(), VerifierError> {
    use RegTypeReq as R;
    match instr {
        // Register I/O
        Instruction::Load { reg } => check_reg(pos, reg.slot(), R::Int, regs)?,
        Instruction::Output { reg } => check_reg(pos, reg.slot(), R::NonUnset, regs)?,

        // ITER and VECLEN accept both VecInt and VecXqmx.
        Instruction::Iter { reg } | Instruction::VecLen { reg } => {
            check_reg(pos, reg.slot(), R::AnyVec, regs)?;
        }

        // Vector operations that specifically require VecInt
        Instruction::VecGet { reg }
        | Instruction::VecSet { reg }
        | Instruction::VecPush { reg } => check_reg(pos, reg.slot(), R::VecInt, regs)?,
        Instruction::Slack { indices, coeffs } => {
            check_reg(pos, indices.slot(), R::VecInt, regs)?;
            check_reg(pos, coeffs.slot(), R::VecInt, regs)?;
        }

        // XQMX coefficient access + high-level constraint helpers: all require Model.
        Instruction::GetLine { reg }
        | Instruction::SetLine { reg }
        | Instruction::AddLine { reg }
        | Instruction::GetQuad { reg }
        | Instruction::SetQuad { reg }
        | Instruction::AddQuad { reg }
        | Instruction::OneHotR { reg }
        | Instruction::OneHotC { reg }
        | Instruction::Exclude { reg }
        | Instruction::Implies { reg } => check_reg(pos, reg.slot(), R::Model, regs)?,

        // XQMX grid operations accept both Model and Sample.
        Instruction::Resize { reg }
        | Instruction::RowFind { reg }
        | Instruction::ColFind { reg }
        | Instruction::RowSum { reg }
        | Instruction::ColSum { reg } => check_reg(pos, reg.slot(), R::Grid, regs)?,

        // Constraint instructions that read multiple registers
        Instruction::Equality {
            model,
            indices,
            coeffs,
        }
        | Instruction::AtLeastW {
            model,
            indices,
            coeffs,
        } => {
            check_reg(pos, model.slot(), R::Model, regs)?;
            check_reg(pos, indices.slot(), R::VecInt, regs)?;
            check_reg(pos, coeffs.slot(), R::VecInt, regs)?;
        }
        Instruction::AtLeast { model, indices } => {
            check_reg(pos, model.slot(), R::Model, regs)?;
            check_reg(pos, indices.slot(), R::VecInt, regs)?;
        }
        Instruction::Reduce { model } => check_reg(pos, model.slot(), R::Model, regs)?,

        // ENERGY requires Model + Sample (sample slot must be Sample, not Model).
        Instruction::Energy { model, sample } => {
            check_reg(pos, model.slot(), R::Model, regs)?;
            check_reg(pos, sample.slot(), R::Sample, regs)?;
        }

        _ => {}
    }
    Ok(())
}

/// Apply register write effects: update `regs` to reflect what `instr` writes.
///
/// Only instructions that change a register's type (or set it for the first
/// time) appear here. Read-modify-write instructions that preserve type
/// (e.g. `VECPUSH`, `SETLINE`) are intentionally absent.
fn apply_writes(instr: &Instruction, regs: &mut [RegType; 256]) {
    // slot is u8 (0–255) and regs has exactly 256 elements, so get_mut() never returns None.
    let set = |regs: &mut [RegType; 256], r: &crate::Register, t: RegType| {
        if let Some(slot) = regs.get_mut(usize::from(r.slot())) {
            *slot = t;
        }
    };
    match instr {
        // STOW and LIDX both write Int to their register.
        Instruction::Stow { reg } | Instruction::Lidx { reg } => set(regs, reg, RegType::Int),
        Instruction::Drop { reg } => set(regs, reg, RegType::Unset),
        // INPUT stores a calldata value whose type is not statically known.
        // LVAL stores the loop element, which is Int or Model depending on
        // loop type -- not statically determinable without tracking loop state.
        Instruction::Input { reg } | Instruction::LVal { reg } => set(regs, reg, RegType::Any),
        // XQMX model allocators
        Instruction::Bqmx { reg } | Instruction::Sqmx { reg } | Instruction::Xqmx { reg } => {
            set(regs, reg, RegType::Model);
        }
        // XQMX sample allocators
        Instruction::Bsmx { reg } | Instruction::Ssmx { reg } | Instruction::Xsmx { reg } => {
            set(regs, reg, RegType::Sample);
        }
        // Integer vector allocators
        Instruction::Vec { reg } | Instruction::VecI { reg } => {
            set(regs, reg, RegType::VecInt);
        }
        // XQMX vector allocator
        Instruction::VecX { reg } => set(regs, reg, RegType::VecXqmx),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// RegisterTypePhase
// ---------------------------------------------------------------------------

/// Forward type-state pass: detects register reads-before-write and
/// type mismatches.
///
/// Maintains a `[RegType; 256]` state array initialised to [`RegType::Unset`]
/// and advances it instruction by instruction. For each instruction:
///
/// 1. **Check reads** -- each register-reading operand is validated against the
///    instruction's type requirement; an error is returned immediately.
/// 2. **Apply writes** -- each register-writing operand updates the state array.
///
/// The scan is linear: no control-flow graph is built. Conditional branches
/// and backward jumps are ignored, so paths through unexecuted branches may
/// produce false negatives (missed errors) but not false positives (spurious
/// errors on valid programs).
///
/// Registers written by `INPUT` or `LVAL` are typed [`RegType::Any`], which
/// satisfies any read requirement; this prevents false positives when the
/// calldata type or loop element type is not statically known.
pub struct RegisterTypePhase;

impl Phase for RegisterTypePhase {
    type Error = VerifierError;

    fn run(&self, program: &Program) -> Result<(), VerifierError> {
        let mut regs = [RegType::Unset; 256];
        let mut stream = InstructionStream::new(program.code());
        while let Some(item) = stream.next_instruction() {
            let (pos, _label, instr) = item.map_err(|e| stream_err(&e))?;
            check_reads(pos, &instr, &regs)?;
            apply_writes(&instr, &mut regs);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// StackEffectPhase
// ---------------------------------------------------------------------------

/// Maximum integer stack depth permitted by the VM spec.
const STACK_LIMIT: usize = 8192;

/// Forward stack-depth pass: detects stack underflow, potential overflow, and
/// non-neutral loop bodies.
///
/// Makes one linear pass, maintaining a running `depth: i64` (starting at 0)
/// and a `loop_stack` of `(opener_offset, entry_depth)` pairs.
///
/// For each instruction the delta from [`Instruction::stack_effect`] is applied:
///
/// - [`StackEffect::Delta`] `d`: if `depth + d < 0`, emit [`VerifierError::StackUnderflow`];
///   if the new depth exceeds the stack limit (8192), emit [`VerifierError::StackOverflowRisk`].
/// - [`StackEffect::Reset`] (`SCLR`): unconditionally sets `depth = 0`.
///
/// Loop tracking (`RANGE`/`ITER`/`NEXT`) is handled after applying the depth
/// delta: on `RANGE`/`ITER`, record `(offset, depth)` on the loop stack; on
/// `NEXT`, pop the entry and compare with the current depth.
///
/// # Limitations
///
/// The scan is linear; conditional branches (`JUMPI1`/`JUMPI2`) are not
/// followed. This means underflow on paths not taken may be missed (false
/// negatives), but no false positives are produced on valid programs.
pub struct StackEffectPhase;

impl Phase for StackEffectPhase {
    type Error = VerifierError;

    fn run(&self, program: &Program) -> Result<(), VerifierError> {
        let mut depth: usize = 0;
        // Each entry: (byte offset of RANGE/ITER opener, depth at loop body entry).
        let mut loop_stack: Vec<(usize, usize)> = Vec::new();
        let mut stream = InstructionStream::new(program.code());

        while let Some(item) = stream.next_instruction() {
            let (pos, _label, instr) = item.map_err(|e| stream_err(&e))?;

            // Apply the stack delta for this instruction.
            match instr.stack_effect() {
                StackEffect::Reset => {
                    depth = 0;
                }
                StackEffect::Delta(d) => {
                    depth = depth
                        .checked_add_signed(isize::from(d))
                        .ok_or(VerifierError::StackUnderflow { offset: pos })?;
                    if depth > STACK_LIMIT {
                        return Err(VerifierError::StackOverflowRisk { offset: pos, depth });
                    }
                }
            }

            match instr {
                Instruction::Range {} | Instruction::Iter { .. } => {
                    loop_stack.push((pos, depth));
                }
                Instruction::Next {} => {
                    if let Some((opener, entry)) = loop_stack.pop()
                        && depth != entry
                    {
                        return Err(VerifierError::LoopStackImbalance {
                            opener,
                            entry,
                            exit: depth,
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Convenience entry point
// ---------------------------------------------------------------------------

/// Run the standard Phase 1 + Phase 2 verifier over `program`.
///
/// Equivalent to `Verifier::default().run(program)`. Runs `CombinedPhase`
/// (structural, jump-target, loop-nesting) followed by [`RegisterTypePhase`]
/// (read-before-write and type-state checks). Returns the first
/// [`VerifierError`] found, or `Ok(())` if all checks pass.
///
/// # Errors
///
/// Returns a [`VerifierError`] on the first violation detected.
///
/// # Examples
///
/// ```rust
/// use xqvm::{verifier, InstructionBuilder};
///
/// let mut b = InstructionBuilder::new();
/// b.emit_push(42).emit_halt();
/// assert!(verifier::verify(&b.build().unwrap()).is_ok());
/// ```
pub fn verify(program: &Program) -> Result<(), VerifierError> {
    Verifier::default().run(program)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::codec;
    use crate::{Instruction, InstructionBuilder, Program, Register};

    fn bytes(instrs: &[Instruction]) -> Vec<u8> {
        instrs.iter().flat_map(codec::encode).collect()
    }

    // --- StructuralPhase ---

    #[test]
    fn structural_accepts_valid_stream() {
        let mut b = InstructionBuilder::new();
        let _ = b.emit_push(1).emit_push(2).emit_add().emit_halt();
        assert!(StructuralPhase.run(&b.build().unwrap()).is_ok());
    }

    #[test]
    fn structural_rejects_truncated_push2() {
        // PUSH2 (0x12) needs 2 operand bytes; supply only 1.
        let prog = Program::new(vec![0x12, 0x00]);
        let err = StructuralPhase.run(&prog).unwrap_err();
        assert_eq!(err.variant_name(), "TruncatedInstruction");
    }

    #[test]
    fn structural_rejects_reserved_opcode() {
        // 0x0D is the reserved gap in the opcode table.
        let err = StructuralPhase.run(&Program::new(vec![0x0D])).unwrap_err();
        assert_eq!(err.variant_name(), "BadOpcode");
    }

    // --- JumpTargetPhase ---

    #[test]
    fn jump_target_accepts_valid_label() {
        // TARGET(.0) then JUMP1 0.
        let code = bytes(&[
            Instruction::Target {},
            Instruction::Jump1 { label: 0 },
            Instruction::Halt {},
        ]);
        assert!(JumpTargetPhase.run(&Program::new(code)).is_ok());
    }

    #[test]
    fn jump1_to_nonexistent_label() {
        // No TARGETs; label 0 is undefined.
        let code = bytes(&[Instruction::Jump1 { label: 0 }, Instruction::Halt {}]);
        let err = JumpTargetPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "UndefinedJumpTarget");
    }

    #[test]
    fn jumpi1_to_nonexistent_label() {
        let code = bytes(&[Instruction::JumpI1 { label: 0 }, Instruction::Halt {}]);
        let err = JumpTargetPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "UndefinedJumpTarget");
    }

    #[test]
    fn jump2_to_nonexistent_label() {
        let code = bytes(&[Instruction::Jump2 { label: 0 }, Instruction::Halt {}]);
        let err = JumpTargetPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "UndefinedJumpTarget");
    }

    #[test]
    fn jump2_label_out_of_range_with_one_target() {
        // One TARGET (id 0); JUMP2 label=1 is out of range.
        let code = bytes(&[
            Instruction::Target {},
            Instruction::Jump2 { label: 1 },
            Instruction::Halt {},
        ]);
        let err = JumpTargetPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "UndefinedJumpTarget");
    }

    // --- LoopNestingPhase ---

    #[test]
    fn loop_nesting_accepts_balanced_range() {
        let mut b = InstructionBuilder::new();
        let _ = b
            .emit_push(0)
            .emit_push(3)
            .emit_range()
            .emit_next()
            .emit_halt();
        assert!(LoopNestingPhase.run(&b.build().unwrap()).is_ok());
    }

    #[test]
    fn unmatched_range_at_eof() {
        let code = bytes(&[Instruction::Range {}, Instruction::Halt {}]);
        let err = LoopNestingPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "UnmatchedLoop");
    }

    #[test]
    fn next_outside_any_loop() {
        let code = bytes(&[Instruction::Next {}, Instruction::Halt {}]);
        let err = LoopNestingPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "NoActiveLoop");
    }

    #[test]
    fn lval_outside_loop() {
        let code = bytes(&[Instruction::LVal { reg: Register(0) }, Instruction::Halt {}]);
        let err = LoopNestingPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "NoActiveLoop");
    }

    #[test]
    fn lidx_outside_loop() {
        let code = bytes(&[Instruction::Lidx { reg: Register(0) }, Instruction::Halt {}]);
        let err = LoopNestingPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "NoActiveLoop");
    }

    #[test]
    fn nested_loops_balanced() {
        // RANGE { RANGE { NEXT } NEXT } HALT
        let code = bytes(&[
            Instruction::Range {},
            Instruction::Range {},
            Instruction::Next {},
            Instruction::Next {},
            Instruction::Halt {},
        ]);
        assert!(LoopNestingPhase.run(&Program::new(code)).is_ok());
    }

    #[test]
    fn nested_loops_outer_unmatched() {
        // RANGE { RANGE { NEXT } HALT -- outer RANGE has no NEXT
        let code = bytes(&[
            Instruction::Range {},
            Instruction::Range {},
            Instruction::Next {},
            Instruction::Halt {},
        ]);
        let err = LoopNestingPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "UnmatchedLoop");
        // Outermost opener is at offset 0.
        assert!(matches!(
            err,
            VerifierError::UnmatchedLoop {
                offset: 0,
                depth: 1
            }
        ));
    }

    // --- RegisterTypePhase ---

    #[test]
    fn reg_type_stow_then_load_ok() {
        let code = bytes(&[
            Instruction::Push1 { val: [7] },
            Instruction::Stow { reg: Register(0) },
            Instruction::Load { reg: Register(0) },
            Instruction::Halt {},
        ]);
        assert!(RegisterTypePhase.run(&Program::new(code)).is_ok());
    }

    #[test]
    fn reg_type_load_unset_register_is_error() {
        let code = bytes(&[Instruction::Load { reg: Register(1) }, Instruction::Halt {}]);
        let err = RegisterTypePhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "ReadUnsetRegister");
    }

    #[test]
    fn reg_type_output_unset_register_is_error() {
        let code = bytes(&[
            Instruction::Push1 { val: [0] }, // output slot index
            Instruction::Output { reg: Register(2) },
            Instruction::Halt {},
        ]);
        let err = RegisterTypePhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "ReadUnsetRegister");
    }

    #[test]
    fn reg_type_drop_clears_register() {
        // STOW r0, DROP r0, LOAD r0 -- Load after Drop should error.
        let code = bytes(&[
            Instruction::Push1 { val: [1] },
            Instruction::Stow { reg: Register(0) },
            Instruction::Drop { reg: Register(0) },
            Instruction::Load { reg: Register(0) },
            Instruction::Halt {},
        ]);
        let err = RegisterTypePhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "ReadUnsetRegister");
    }

    #[test]
    fn reg_type_wrong_type_for_load() {
        // BQMX writes Model to r0; LOAD expects Int.
        let code = bytes(&[
            Instruction::Push1 { val: [4] }, // size
            Instruction::Bqmx { reg: Register(0) },
            Instruction::Load { reg: Register(0) },
            Instruction::Halt {},
        ]);
        let err = RegisterTypePhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "RegisterTypeMismatch");
    }

    #[test]
    fn reg_type_input_satisfies_any_read() {
        // INPUT writes Any; subsequent LOAD should not error.
        let code = bytes(&[
            Instruction::Push1 { val: [0] }, // calldata index
            Instruction::Input { reg: Register(0) },
            Instruction::Load { reg: Register(0) },
            Instruction::Halt {},
        ]);
        assert!(RegisterTypePhase.run(&Program::new(code)).is_ok());
    }

    #[test]
    fn reg_type_bqmx_then_getline_ok() {
        let code = bytes(&[
            Instruction::Push1 { val: [4] },
            Instruction::Bqmx { reg: Register(0) },
            Instruction::Push1 { val: [0] },
            Instruction::GetLine { reg: Register(0) },
            Instruction::Halt {},
        ]);
        assert!(RegisterTypePhase.run(&Program::new(code)).is_ok());
    }

    #[test]
    fn reg_type_veci_then_getline_mismatch() {
        // VECI writes VecInt to r0; GETLINE expects Model.
        let code = bytes(&[
            Instruction::VecI { reg: Register(0) },
            Instruction::Push1 { val: [0] },
            Instruction::GetLine { reg: Register(0) },
            Instruction::Halt {},
        ]);
        let err = RegisterTypePhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "RegisterTypeMismatch");
    }

    #[test]
    fn reg_type_energy_wrong_sample_slot() {
        // BQMX r0=Model, BSMX r1=Sample, ENERGY with model=r1, sample=r0.
        // r1 is Sample (ok for model slot? no -- model slot requires Model).
        let code = bytes(&[
            Instruction::Push1 { val: [2] },
            Instruction::Bqmx { reg: Register(0) }, // r0 = Model
            Instruction::Push1 { val: [2] },
            Instruction::Bsmx { reg: Register(1) }, // r1 = Sample
            // Energy with model=r1(Sample) -- should fail
            Instruction::Energy {
                model: Register(1),
                sample: Register(0),
            },
            Instruction::Halt {},
        ]);
        let err = RegisterTypePhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "RegisterTypeMismatch");
    }

    #[test]
    fn reg_type_correct_energy_program() {
        let code = bytes(&[
            Instruction::Push1 { val: [2] },
            Instruction::Bqmx { reg: Register(0) }, // r0 = Model
            Instruction::Push1 { val: [2] },
            Instruction::Bsmx { reg: Register(1) }, // r1 = Sample
            Instruction::Energy {
                model: Register(0),
                sample: Register(1),
            },
            Instruction::Halt {},
        ]);
        assert!(RegisterTypePhase.run(&Program::new(code)).is_ok());
    }

    #[test]
    fn reg_type_veclen_accepts_vec_xqmx() {
        let code = bytes(&[
            Instruction::VecX { reg: Register(0) }, // r0 = VecXqmx
            Instruction::VecLen { reg: Register(0) },
            Instruction::Halt {},
        ]);
        assert!(RegisterTypePhase.run(&Program::new(code)).is_ok());
    }

    #[test]
    fn reg_type_resize_accepts_model_or_sample() {
        // RESIZE accepts both Model and Sample via the Grid requirement.
        let code_model = bytes(&[
            Instruction::Push1 { val: [4] },
            Instruction::Bqmx { reg: Register(0) },
            Instruction::Push1 { val: [8] },
            Instruction::Push1 { val: [8] },
            Instruction::Resize { reg: Register(0) },
            Instruction::Halt {},
        ]);
        assert!(RegisterTypePhase.run(&Program::new(code_model)).is_ok());

        let code_sample = bytes(&[
            Instruction::Push1 { val: [4] },
            Instruction::Bsmx { reg: Register(0) },
            Instruction::Push1 { val: [8] },
            Instruction::Push1 { val: [8] },
            Instruction::Resize { reg: Register(0) },
            Instruction::Halt {},
        ]);
        assert!(RegisterTypePhase.run(&Program::new(code_sample)).is_ok());
    }

    // --- Verifier composition ---

    #[test]
    fn default_verifier_passes_valid_program() {
        let mut b = InstructionBuilder::new();
        let _ = b.emit_push(10).emit_push(32).emit_add().emit_halt();
        assert!(verify(&b.build().unwrap()).is_ok());
    }

    #[test]
    fn custom_verifier_only_jump_phase() {
        // An unmatched RANGE is not caught if LoopNestingPhase is not included.
        let code = bytes(&[Instruction::Range {}, Instruction::Halt {}]);
        let result = Verifier::new()
            .with_phase(JumpTargetPhase)
            .run(&Program::new(code));
        assert!(
            result.is_ok(),
            "jump-only verifier should not catch loop errors"
        );
    }

    #[test]
    fn custom_verifier_structural_then_loop() {
        // A bad opcode is caught by StructuralPhase before LoopNestingPhase runs.
        let prog = Program::new(vec![0x0D]);
        let err = Verifier::new()
            .with_phase(StructuralPhase)
            .with_phase(LoopNestingPhase)
            .run(&prog)
            .unwrap_err();
        assert_eq!(err.variant_name(), "BadOpcode");
    }

    // --- StackEffectPhase ---

    #[test]
    fn stack_effect_underflow_on_pop_empty() {
        // POP with nothing on the stack.
        let code = bytes(&[Instruction::Pop {}, Instruction::Halt {}]);
        let err = StackEffectPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "StackUnderflow");
    }

    #[test]
    fn stack_effect_underflow_on_binary_op() {
        // ADD on an empty stack: delta = -1, depth = 0 → depth + delta < 0 → underflow.
        // Note: the conservative scan uses net delta, so ADD with depth=1 (one item) would
        // not be caught here -- that is a documented limitation of the linear scan.
        let code = bytes(&[Instruction::Add {}, Instruction::Halt {}]);
        let err = StackEffectPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "StackUnderflow");
    }

    #[test]
    fn stack_effect_valid_push_pop() {
        let code = bytes(&[
            Instruction::Push1 { val: [42] },
            Instruction::Pop {},
            Instruction::Halt {},
        ]);
        assert!(StackEffectPhase.run(&Program::new(code)).is_ok());
    }

    #[test]
    fn stack_effect_sclr_resets_depth() {
        // PUSH, PUSH, SCLR, then POP would underflow — SCLR drops both items.
        let code = bytes(&[
            Instruction::Push1 { val: [1] },
            Instruction::Push1 { val: [2] },
            Instruction::Sclr {},
            Instruction::Pop {},
            Instruction::Halt {},
        ]);
        let err = StackEffectPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "StackUnderflow");
    }

    #[test]
    fn stack_effect_sclr_leaves_clean_stack() {
        // PUSH, PUSH, SCLR -- depth is 0 afterwards; HALT is fine.
        let code = bytes(&[
            Instruction::Push1 { val: [1] },
            Instruction::Push1 { val: [2] },
            Instruction::Sclr {},
            Instruction::Halt {},
        ]);
        assert!(StackEffectPhase.run(&Program::new(code)).is_ok());
    }

    // SCLR inside a loop resets the depth counter to 0 unconditionally. If the
    // depth at loop body entry was N > 0, the exit depth will be 0 != N and
    // LoopStackImbalance is raised. The error message says "loop has non-zero
    // stack effect", which is technically correct but the root cause is a global
    // stack reset mid-iteration rather than an unmatched push or pop.
    #[test]
    fn sclr_inside_loop_body_causes_imbalance() {
        let code = bytes(&[
            Instruction::Push1 { val: [1] }, // extra item -- entry depth before RANGE = 3
            Instruction::Push1 { val: [0] }, // start
            Instruction::Push1 { val: [3] }, // count
            Instruction::Range {},           // pops 2; entry depth recorded = 1
            Instruction::Sclr {},            // resets depth to 0; exit depth = 0 != 1
            Instruction::Next {},
            Instruction::Halt {},
        ]);
        let err = StackEffectPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "LoopStackImbalance");
    }

    #[test]
    fn loop_body_neutral_passes() {
        // PUSH start, PUSH count, RANGE, NEXT, HALT -- body is empty, net effect = 0.
        let code = bytes(&[
            Instruction::Push1 { val: [0] },
            Instruction::Push1 { val: [3] },
            Instruction::Range {},
            Instruction::Next {},
            Instruction::Halt {},
        ]);
        assert!(StackEffectPhase.run(&Program::new(code)).is_ok());
    }

    #[test]
    fn loop_body_push_causes_imbalance() {
        // Loop body does a PUSH but no matching POP: each iteration leaks one item.
        let code = bytes(&[
            Instruction::Push1 { val: [0] },
            Instruction::Push1 { val: [3] },
            Instruction::Range {},
            Instruction::Push1 { val: [99] }, // unmatched push inside loop
            Instruction::Next {},
            Instruction::Halt {},
        ]);
        let err = StackEffectPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "LoopStackImbalance");
    }

    #[test]
    fn loop_body_pop_causes_imbalance() {
        // Loop body pops the loop's own operands off the stack.
        // Entry depth after RANGE's -2 = 0. POP inside would underflow -- caught as underflow.
        // Instead use a loop that has 1 extra item before RANGE.
        // depth before RANGE = 3 (start=0, count=3, extra=1 → no, RANGE pops 2 → depth=1)
        // body pops 1 → depth=0, NEXT expects depth=1 → LoopStackImbalance
        let code = bytes(&[
            Instruction::Push1 { val: [1] }, // extra item
            Instruction::Push1 { val: [0] }, // start
            Instruction::Push1 { val: [3] }, // count
            Instruction::Range {},           // pops 2, depth = 1
            Instruction::Pop {},             // pops extra, depth = 0
            Instruction::Next {},            // expects depth = 1
            Instruction::Halt {},
        ]);
        let err = StackEffectPhase.run(&Program::new(code)).unwrap_err();
        assert_eq!(err.variant_name(), "LoopStackImbalance");
    }

    #[test]
    fn nested_loops_both_neutral() {
        let code = bytes(&[
            Instruction::Push1 { val: [0] },
            Instruction::Push1 { val: [2] },
            Instruction::Range {},
            Instruction::Push1 { val: [0] },
            Instruction::Push1 { val: [2] },
            Instruction::Range {},
            Instruction::Next {},
            Instruction::Next {},
            Instruction::Halt {},
        ]);
        assert!(StackEffectPhase.run(&Program::new(code)).is_ok());
    }

    #[test]
    fn stack_effect_on_all_variants_does_not_panic() {
        // Smoke test: stack_effect() must not panic for any instruction variant.
        use crate::bytecode::types::Register;
        let instrs: &[Instruction] = &[
            Instruction::Target {},
            Instruction::Jump1 { label: 0 },
            Instruction::JumpI1 { label: 0 },
            Instruction::Nop {},
            Instruction::Halt {},
            Instruction::Sclr {},
            Instruction::Add {},
            Instruction::Energy {
                model: Register(0),
                sample: Register(1),
            },
        ];
        for instr in instrs {
            let _ = instr.stack_effect();
        }
    }
}
