# Copyright (C) 2026 Postquant Labs Incorporated
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU Affero General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU Affero General Public License for more details.
#
# You should have received a copy of the GNU Affero General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.
#
# SPDX-License-Identifier: AGPL-3.0-or-later

"""
Program and Instruction types, plus construction and execution helpers.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from .opcodes import Opcode


@dataclass(frozen=True)
class Instruction:
    """
    A single XQVM instruction.

    Attributes:
        opcode: The operation to perform
        operands: Tuple of operand values (immediates, register indices, target IDs)
        line: Source line number for debugging (0 if unknown)
    """

    opcode: Opcode
    operands: tuple[int, ...] = ()
    line: int = 0


@dataclass
class Program:
    """
    A complete XQVM program.

    Attributes:
        instructions: List of instructions to execute
        name: Optional program name for debugging
    """

    instructions: list[Instruction] = field(default_factory=list)
    name: str = ""

    def __len__(self) -> int:
        return len(self.instructions)

    def __getitem__(self, index: int) -> Instruction:
        return self.instructions[index]


def make_program(instructions: list[Instruction]) -> Program:
    """Build a Program from a list of Instructions."""
    return Program(instructions)


def run_program(instructions: list[Instruction], input_data: dict[int, Any] | None = None):
    """Build and execute a program, returning executor for state inspection."""
    from .executor import Executor

    prog = make_program(instructions)
    ex = Executor()
    ex.execute(prog, input_data)
    return ex


def program_from_bytecode(bytecode: bytes, name: str = "") -> Program:
    """Decode raw ``.xqb`` bytecode into an executable ``Program``.

    Pure-Python decoder — no FFI dependency. The wire format is a flat
    stream of ``[opcode_u8, operand_bytes...]`` with no header or length
    prefixes. Each opcode's ``OpcodeMeta.operand_count`` determines the
    number of trailing bytes.
    """
    instructions: list[Instruction] = []
    pos = 0
    while pos < len(bytecode):
        code = bytecode[pos]
        opcode = Opcode.from_code(code)
        if opcode is None:
            raise ValueError(f"unknown opcode 0x{code:02X} at byte offset {pos}")
        n = opcode.meta.operand_count
        if pos + 1 + n > len(bytecode):
            raise ValueError(
                f"truncated operands for {opcode.name} at byte offset {pos}: "
                f"need {n} bytes, have {len(bytecode) - pos - 1}"
            )
        operands = tuple(bytecode[pos + 1 : pos + 1 + n])
        instructions.append(Instruction(opcode, operands))
        pos += 1 + n
    return Program(instructions=instructions, name=name)


def program_from_xqasm(source: str, name: str = "") -> Program:
    """Parse ``.xqasm`` source via the Rust xqasm crate and wrap the result.

    This is the single entry point xqvm-py uses to turn assembly text
    into an executable ``Program``. The actual parsing + assembly is
    done by ``xqffi.asm.parse_xqasm`` (a pyo3 binding to Rust
    ``xqasm``); this helper just adapts the wire dict into the Python
    dataclass layout the executor expects.

    The ``program_from_xqasm`` / ``Instruction`` / ``Program`` trio
    replaces what used to be ``xqvm.assembler.assemble(src).program``
    — xqvm-py no longer ships its own assembler.
    """
    from xqffi.asm import parse_xqasm

    wire = parse_xqasm(source)
    instructions = [Instruction(Opcode.from_code(code), tuple(ops), int(pc)) for code, ops, pc in wire["instructions"]]
    return Program(instructions=instructions, name=name)
