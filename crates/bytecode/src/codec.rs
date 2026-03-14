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

//! Binary codec for XQVM instructions.
//!
//! [`Instruction`] is encoded as a sequence `[opcode: u8, operands...]`:
//!
//! * **Sequential formats** (postcard, bincode, ...): opcode as one raw byte,
//!   then operand fields in declaration order.
//! * **Sequence-based formats** (JSON arrays, ...): `[opcode, val, ...]`.
//!
//! The opcode is always a fixint `u8` (one byte).  Integer operands are
//! encoded by the underlying serializer (`i16` and `i64` are zigzag varint in
//! postcard; plain numbers in JSON).
//!
//! # Examples
//!
//! ```rust
//! use aglais_xqvm_bytecode::types::{Instruction, Register};
//! use aglais_xqvm_bytecode::codec;
//!
//! let instr = Instruction::Push { imm: 42 };
//! let bytes = codec::encode(&instr);
//! assert_eq!(bytes[0], 0x10); // PUSH opcode byte
//!
//! let (decoded, consumed) = codec::decode(&bytes).unwrap();
//! assert_eq!(decoded, instr);
//! assert_eq!(consumed, bytes.len());
//! ```

use std::fmt;

use serde::de::{self, SeqAccess, Visitor};
use serde::ser::SerializeTuple;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::types::{Instruction, Register};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Encode a single instruction to a `Vec<u8>` in the wire format
/// `[opcode: u8][operand fields]`.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_bytecode::types::Instruction;
/// use aglais_xqvm_bytecode::codec;
///
/// assert_eq!(codec::encode(&Instruction::Halt {}), [0x0F]);
/// assert_eq!(codec::encode(&Instruction::Nop {}),  [0x00]);
/// ```
pub fn encode(instr: &Instruction) -> Vec<u8> {
    // SAFETY: postcard serialization of a statically-known Rust type with no
    // I/O and a fixed-capacity allocator is infallible -- it only fails on
    // I/O errors or allocator exhaustion, neither of which can occur here.
    postcard::to_allocvec(instr).unwrap_or_else(|_| unreachable!())
}

/// Decode a single instruction from the start of `bytes`.
///
/// Returns `(instruction, bytes_consumed)` on success.
///
/// # Errors
///
/// Returns a [`postcard::Error`] when the byte slice is too short or the
/// opcode byte is not a known XQVM opcode.
///
/// # Examples
///
/// ```rust
/// use aglais_xqvm_bytecode::types::{Instruction, Register};
/// use aglais_xqvm_bytecode::codec;
///
/// // LOAD r3 -> [0x14, 0x03]
/// let bytes: &[u8] = &[0x14, 0x03];
/// let (instr, n) = codec::decode(bytes).unwrap();
/// assert_eq!(instr, Instruction::Load { reg: Register(3) });
/// assert_eq!(n, 2);
/// ```
pub fn decode(bytes: &[u8]) -> Result<(Instruction, usize), postcard::Error> {
    let (instr, remaining) = postcard::take_from_bytes(bytes)?;
    Ok((instr, bytes.len() - remaining.len()))
}

// ---------------------------------------------------------------------------
// Register: serialize as a single raw byte, deserialize from a single byte
// ---------------------------------------------------------------------------

impl Serialize for Register {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.0)
    }
}

impl<'de> Deserialize<'de> for Register {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u8::deserialize(deserializer).map(Self)
    }
}

// ---------------------------------------------------------------------------
// Instruction: sequence [opcode: u8, operands...]
// ---------------------------------------------------------------------------

macro_rules! impl_instruction_serde {
    ( $( ($code:literal, $variant:ident, $mnem:literal, $doc:literal,
          {$($fname:ident: $ftype:ty),*}) ),* $(,)? ) => {

        // ---- Serialize -------------------------------------------------
        //
        // Each variant becomes a fixed-length tuple:
        //   (opcode: u8, field0, field1, ...)
        //
        // postcard: [opcode raw byte][field bytes...]
        // JSON:     [opcode, val, ...]

        impl Serialize for Instruction {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                match self {
                    $(
                        Self::$variant { $($fname,)* } => {
                            // Count operand fields at compile time.
                            const N: usize = 1 $( + { let _ = stringify!($fname); 1 })*;
                            let mut tup = serializer.serialize_tuple(N)?;
                            tup.serialize_element(&($code as u8))?;
                            $( tup.serialize_element($fname)?; )*
                            tup.end()
                        }
                    )*
                }
            }
        }

        // ---- Deserialize -----------------------------------------------
        //
        // Read the opcode element first, then dispatch to read the operands
        // for the matching variant.  Works for any format that calls visit_seq
        // (postcard, bincode, JSON arrays, ...).

        impl<'de> Deserialize<'de> for Instruction {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct InstrVisitor;

                impl<'de> Visitor<'de> for InstrVisitor {
                    type Value = Instruction;

                    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        write!(f, "an XQVM instruction as [opcode: u8, operands...]")
                    }

                    fn visit_seq<S: SeqAccess<'de>>(
                        self,
                        mut seq: S,
                    ) -> Result<Instruction, S::Error> {
                        let opcode: u8 = seq
                            .next_element()?
                            .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                        match opcode {
                            $(
                                $code => {
                                    $(
                                        let $fname: $ftype = seq
                                            .next_element()?
                                            .ok_or_else(|| de::Error::missing_field(
                                                stringify!($fname)
                                            ))?;
                                    )*
                                    Ok(Instruction::$variant { $($fname,)* })
                                }
                            )*
                            _ => Err(de::Error::custom(
                                format!("unknown XQVM opcode 0x{opcode:02X}")
                            )),
                        }
                    }
                }

                // Binary formats (postcard, bincode, ...): serialize_tuple
                // writes no length prefix, so deserialize_tuple is the
                // matching call -- it reads elements on demand without
                // consuming a length varint.
                //
                // Human-readable formats (JSON, ...): the value is a JSON
                // array of variable length; deserialize_seq is correct there.
                //
                // The length hint is the widest variant: opcode (1) plus the
                // number of operand fields in each variant.
                let max_fields = [$(1usize $( + { let _ = stringify!($fname); 1 })*),*]
                    .into_iter()
                    .max()
                    .unwrap_or(1);
                if deserializer.is_human_readable() {
                    deserializer.deserialize_seq(InstrVisitor)
                } else {
                    deserializer.deserialize_tuple(max_fields, InstrVisitor)
                }
            }
        }
    };
}

opcodes!(impl_instruction_serde);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Register;

    macro_rules! all_instructions {
        ( $( ($code:literal, $variant:ident, $mnem:literal, $doc:literal,
              {$($fname:ident: $ftype:ty),*}) ),* $(,)? ) => {
            [ $( Instruction::$variant { $($fname: <$ftype as Default>::default(),)* } ),* ]
        };
    }

    // -----------------------------------------------------------------------
    // Postcard (binary) roundtrip tests
    // -----------------------------------------------------------------------

    #[test]
    fn encode_decode_roundtrip_all_68() {
        for instr in opcodes!(all_instructions) {
            let bytes = encode(&instr);
            let (decoded, consumed) = decode(&bytes).expect("decode failed");
            assert_eq!(decoded, instr, "roundtrip mismatch for {instr:?}");
            assert_eq!(
                consumed,
                bytes.len(),
                "consumed != encoded length for {instr:?}"
            );
        }
    }

    #[test]
    fn halt_is_one_byte() {
        assert_eq!(encode(&Instruction::Halt {}), [0x0F]);
    }

    #[test]
    fn nop_is_one_byte() {
        assert_eq!(encode(&Instruction::Nop {}), [0x00]);
    }

    #[test]
    fn push_zero_is_two_bytes() {
        // opcode 0x10, zigzag(0) = 0x00
        assert_eq!(encode(&Instruction::Push { imm: 0 }), [0x10, 0x00]);
    }

    #[test]
    fn push_positive_zigzag_varint() {
        // zigzag(1) = 2, varint([0x02])
        assert_eq!(encode(&Instruction::Push { imm: 1 }), [0x10, 0x02]);
    }

    #[test]
    fn push_negative_zigzag_varint() {
        // zigzag(-1) = 1, varint([0x01])
        assert_eq!(encode(&Instruction::Push { imm: -1 }), [0x10, 0x01]);
    }

    #[test]
    fn jump_offset_zigzag_varint() {
        // zigzag(-10) = 19, varint([0x13])
        let bytes = encode(&Instruction::Jump { offset: -10i16 });
        assert_eq!(bytes, [0x02, 0x13]);
    }

    #[test]
    fn energy_two_register_bytes() {
        // Register serialises as raw u8
        let bytes = encode(&Instruction::Energy {
            model: Register(2),
            sample: Register(3),
        });
        assert_eq!(bytes, [0x7F, 2, 3]);
    }

    #[test]
    fn unknown_opcode_returns_error() {
        // 0x08 is a gap opcode (single-byte fixint, not assigned)
        assert!(decode(&[0x08u8]).is_err());
    }

    #[test]
    fn truncated_input_returns_error() {
        // PUSH opcode without the required varint operand byte
        assert!(decode(&[0x10u8]).is_err());
    }

    #[test]
    fn encode_sequence_then_decode_each() {
        let program = [
            Instruction::Push { imm: 5 },
            Instruction::Push { imm: 3 },
            Instruction::Add {},
            Instruction::Halt {},
        ];
        let mut buf = Vec::new();
        for instr in &program {
            buf.extend_from_slice(&encode(instr));
        }
        let mut pos = 0usize;
        for expected in &program {
            let (got, n) = decode(&buf[pos..]).unwrap();
            assert_eq!(got, *expected);
            pos += n;
        }
        assert_eq!(pos, buf.len());
    }

    // -----------------------------------------------------------------------
    // JSON (sequence-based) roundtrip tests
    //
    // Instructions serialize as JSON arrays: [opcode, val, ...]
    // -----------------------------------------------------------------------

    #[test]
    fn json_roundtrip_nop() {
        // Nop: opcode 0x00 -> [0]
        let instr = Instruction::Nop {};
        let json = serde_json::to_string(&instr).unwrap();
        assert_eq!(json, "[0]");
        assert_eq!(serde_json::from_str::<Instruction>(&json).unwrap(), instr);
    }

    #[test]
    fn json_roundtrip_push() {
        // Push: opcode 0x10=16, imm -> [16, -99]
        let instr = Instruction::Push { imm: -99 };
        let json = serde_json::to_string(&instr).unwrap();
        assert_eq!(json, "[16,-99]");
        assert_eq!(serde_json::from_str::<Instruction>(&json).unwrap(), instr);
    }

    #[test]
    fn json_roundtrip_load() {
        // Load: opcode 0x14=20, reg -> [20, 7]
        let instr = Instruction::Load { reg: Register(7) };
        let json = serde_json::to_string(&instr).unwrap();
        assert_eq!(json, "[20,7]");
        assert_eq!(serde_json::from_str::<Instruction>(&json).unwrap(), instr);
    }

    #[test]
    fn json_roundtrip_jump() {
        // Jump: opcode 0x02=2, offset -> [2, -10]
        let instr = Instruction::Jump { offset: -10i16 };
        let json = serde_json::to_string(&instr).unwrap();
        assert_eq!(json, "[2,-10]");
        assert_eq!(serde_json::from_str::<Instruction>(&json).unwrap(), instr);
    }

    #[test]
    fn json_roundtrip_energy() {
        // Energy: opcode 0x7F=127, model, sample -> [127, 2, 3]
        let instr = Instruction::Energy {
            model: Register(2),
            sample: Register(3),
        };
        let json = serde_json::to_string(&instr).unwrap();
        assert_eq!(json, "[127,2,3]");
        assert_eq!(serde_json::from_str::<Instruction>(&json).unwrap(), instr);
    }

    #[test]
    fn json_roundtrip_all_68() {
        for instr in opcodes!(all_instructions) {
            let json = serde_json::to_string(&instr)
                .unwrap_or_else(|e| panic!("serialize failed for {instr:?}: {e}"));
            let decoded: Instruction = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("deserialize failed for {instr:?}: {e}"));
            assert_eq!(decoded, instr, "roundtrip mismatch for {instr:?}");
        }
    }

    #[test]
    fn json_unknown_opcode_returns_error() {
        assert!(serde_json::from_str::<Instruction>("[254]").is_err());
    }

    #[test]
    fn json_register_out_of_range_returns_error() {
        // LOAD opcode=20, reg=256 overflows u8
        assert!(serde_json::from_str::<Instruction>("[20, 256]").is_err());
    }
}
