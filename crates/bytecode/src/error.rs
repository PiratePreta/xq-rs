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

//! Public error types for the XQVM bytecode crate.
//!
//! All error types produced at the crate boundary are re-exported here under
//! stable, fully-qualified names.
//!
//! | Type | Produced by |
//! |---|---|
//! | [`BuilderError`] | [`InstructionBuilder::build`](crate::InstructionBuilder::build) |
//! | [`StreamError`] | [`InstructionStream`](crate::InstructionStream) iteration / seek |

/// Error returned by [`InstructionBuilder::build`](crate::InstructionBuilder::build).
///
/// See [`BuilderError`] for variant documentation.
pub use crate::builder::Error as BuilderError;

/// Error returned by [`InstructionStream`](crate::InstructionStream) iteration and seek.
///
/// See [`StreamError`] for variant documentation.
pub use crate::stream::Error as StreamError;

/// Error returned when an unknown byte is decoded as an [`Opcode`](crate::Opcode).
///
/// The inner `u8` is the unrecognised byte value.
pub use crate::types::DecodeError as OpcodeDecodeError;
