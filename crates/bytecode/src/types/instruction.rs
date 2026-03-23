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

use super::Opcode;

// ---------------------------------------------------------------------------
// Macro-generated Instruction enum
// ---------------------------------------------------------------------------

macro_rules! impl_instruction {
    (
        $( ($code:literal, $variant:ident, $mnem:literal, $doc:literal, {$($fname:ident: $ftype:ty),*}) ),*
        $(,)?
    ) => {
        /// A fully decoded XQVM instruction with its operands.
        ///
        /// All variants use named struct syntax. Unit-like instructions (no
        /// operands) use an empty field list (`Nop {}`). This uniform
        /// representation allows the entire enum to be generated from the
        /// [`opcodes!`](crate::opcodes) x-macro without any special-casing.
        ///
        /// Use [`Instruction::opcode`] to recover the corresponding
        /// [`Opcode`], or [`Instruction::mnemonic`] for the assembly string.
        ///
        /// # Examples
        ///
        /// ```rust
        /// use aglais_xqvm_bytecode::types::Instruction;
        /// use aglais_xqvm_bytecode::types::Register;
        /// use aglais_xqvm_bytecode::types::Opcode;
        ///
        /// let instr = Instruction::PushC0 {};
        /// assert_eq!(instr.opcode(), Opcode::PushC0);
        /// assert_eq!(instr.mnemonic(), "PUSHC_0");
        ///
        /// let instr = Instruction::Energy {
        ///     model:  Register(0),
        ///     sample: Register(1),
        /// };
        /// assert_eq!(instr.opcode(), Opcode::Energy);
        /// ```
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Instruction {
            $(
                #[doc = $doc]
                $variant {
                    $(
                        #[allow(missing_docs)]
                        $fname: $ftype,
                    )*
                },
            )*
        }

        impl Instruction {
            /// Return the [`Opcode`] corresponding to this instruction.
            ///
            /// # Examples
            ///
            /// ```rust
            /// use aglais_xqvm_bytecode::types::Instruction;
            /// use aglais_xqvm_bytecode::types::Register;
            /// use aglais_xqvm_bytecode::types::Opcode;
            ///
            /// assert_eq!(Instruction::Add {}.opcode(), Opcode::Add);
            /// assert_eq!(Instruction::Stow { reg: Register(7) }.opcode(), Opcode::Stow);
            /// ```
            pub fn opcode(&self) -> Opcode {
                match self {
                    $( Self::$variant { .. } => Opcode::$variant, )*
                }
            }

            /// Return the uppercase assembly mnemonic for this instruction.
            ///
            /// Delegates to [`Opcode::mnemonic`].
            ///
            /// # Examples
            ///
            /// ```rust
            /// use aglais_xqvm_bytecode::types::Instruction;
            ///
            /// assert_eq!(Instruction::Halt {}.mnemonic(), "HALT");
            /// ```
            pub fn mnemonic(&self) -> &'static str {
                self.opcode().mnemonic()
            }
        }
    };
}

opcodes!(impl_instruction);

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Register;

    // Build a flat array of (Instruction, expected_opcode, mnemonic) triples
    // from the x-macro table so every variant is covered automatically.
    // Each operand is zero-initialised: Register(0), 0i16, 0i64.
    macro_rules! all_instruction_opcode_pairs {
        (
            $( ($code:literal, $variant:ident, $mnem:literal, $doc:literal, {$($fname:ident: $ftype:ty),*}) ),*
            $(,)?
        ) => {
            [
                $(
                    (
                        Instruction::$variant {
                            $( $fname: <$ftype as Default>::default(), )*
                        },
                        Opcode::$variant,
                        $mnem,
                    )
                ),*
            ]
        };
    }

    #[test]
    fn opcode_method_covers_all_variants() {
        for (instr, expected, _) in opcodes!(all_instruction_opcode_pairs) {
            assert_eq!(instr.opcode(), expected, "opcode mismatch for {instr:?}");
        }
    }

    #[test]
    fn mnemonic_method_covers_all_variants() {
        for (instr, _, expected_mnem) in opcodes!(all_instruction_opcode_pairs) {
            assert_eq!(
                instr.mnemonic(),
                expected_mnem,
                "mnemonic mismatch for {instr:?}"
            );
        }
    }

    #[test]
    fn instruction_count_is_76() {
        assert_eq!(opcodes!(all_instruction_opcode_pairs).len(), 76);
    }

    #[test]
    fn energy_named_fields() {
        let e = Instruction::Energy {
            model: Register(0),
            sample: Register(1),
        };
        assert_eq!(e.opcode(), Opcode::Energy);
        if let Instruction::Energy { model, sample } = e {
            assert_eq!(model.slot(), 0);
            assert_eq!(sample.slot(), 1);
        } else {
            panic!("pattern match failed");
        }
    }
}
