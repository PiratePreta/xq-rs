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

//! Runtime error types for the XQVM interpreter.
use thiserror::Error;

/// Errors that can occur during XQVM bytecode execution.
#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    /// The value stack was empty when a pop was attempted.
    #[error("stack underflow at byte {pos:#06x}")]
    StackUnderflow { pos: usize },

    /// A register held the wrong value kind for the operation.
    #[error("register r{reg} holds {got}, expected {expected}")]
    RegisterType {
        reg: u8,
        expected: &'static str,
        got: &'static str,
    },

    /// Division or modulo by zero.
    #[error("division by zero at byte {pos:#06x}")]
    DivisionByZero { pos: usize },

    /// Vec index out of bounds.
    #[error("vec index {index} out of bounds (len {len}) at byte {pos:#06x}")]
    IndexOutOfBounds { pos: usize, index: i64, len: usize },

    /// NEXT or LVAL executed outside a loop.
    #[error("loop instruction at byte {pos:#06x} with no active loop")]
    NoActiveLoop { pos: usize },

    /// Jump target is outside the bytecode buffer.
    #[error("jump from {pos:#06x} to {target:#010x} is out of bounds")]
    BadJumpTarget { pos: usize, target: usize },

    /// The bytecode stream returned a decode error.
    #[error("bad opcode {byte:#04x} at byte {pos:#06x}")]
    BadOpcode { pos: usize, byte: u8 },

    /// The bytecode stream hit a truncated instruction.
    #[error("truncated instruction at byte {pos:#06x}")]
    TruncatedInstruction { pos: usize },

    /// Calldata index out of range.
    #[error("calldata index {index} out of range (len {len})")]
    CallDataIndex { index: i64, len: usize },

    /// Output index out of range.
    #[error("output index {index} out of range (len {len})")]
    OutputIndex { index: i64, len: usize },

    /// The model and sample sizes do not match.
    #[error("model has {model_size} variables but sample has {sample_len}")]
    SizeMismatch {
        model_size: usize,
        sample_len: usize,
    },

    /// Execution exceeded the configured step limit.
    #[error("step limit of {limit} exceeded")]
    StepLimitExceeded { limit: u64 },

    /// Left shift amount is negative or too large.
    #[error("invalid shift amount {amount} at byte {pos:#06x}")]
    InvalidShift { pos: usize, amount: i64 },

    /// The RESIZE instruction received non-positive dimensions.
    #[error("invalid grid dimensions {rows}x{cols} at byte {pos:#06x}")]
    InvalidGridDimensions { pos: usize, rows: i64, cols: i64 },
}

impl From<aglais_xqvm_bytecode::stream::Error> for Error {
    fn from(e: aglais_xqvm_bytecode::stream::Error) -> Self {
        use aglais_xqvm_bytecode::stream::Error as SE;
        match e {
            SE::UnknownOpcode { offset, byte } => Self::BadOpcode { pos: offset, byte },
            SE::TruncatedInstruction { offset } => Self::TruncatedInstruction { pos: offset },
            SE::SeekOutOfBounds { target, .. } => Self::BadJumpTarget { pos: 0, target },
        }
    }
}
