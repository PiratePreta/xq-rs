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

"""Tests for xqvm_py.program — construction and bytecode decoding."""

from __future__ import annotations

from xqffi.asm import assemble_source
from xqvm_py import program_from_bytecode, program_from_xqasm


def test_bytecode_round_trip():
    """Assemble source → bytecode → decode produces the same program as parsing source directly."""
    source = "PUSH 7\nPUSH 5\nADD\nSTOW r0\nPUSH 0\nOUTPUT r0\nHALT\n"
    bytecode = assemble_source(source)

    from_asm = program_from_xqasm(source)
    from_bc = program_from_bytecode(bytecode)

    assert len(from_asm) == len(from_bc)
    for i, (a, b) in enumerate(zip(from_asm.instructions, from_bc.instructions)):
        assert a.opcode == b.opcode, f"instruction {i}: opcode mismatch {a.opcode} != {b.opcode}"
        assert a.operands == b.operands, f"instruction {i}: operands mismatch {a.operands} != {b.operands}"


def test_bytecode_decode_empty():
    """Empty bytecode produces an empty program."""
    prog = program_from_bytecode(b"")
    assert len(prog) == 0


def test_bytecode_decode_unknown_opcode():
    """Unknown opcode byte raises ValueError."""
    import pytest

    with pytest.raises(ValueError, match="unknown opcode 0x0D"):
        program_from_bytecode(bytes([0x0D]))


def test_bytecode_decode_truncated():
    """Truncated operands raise ValueError."""
    import pytest

    with pytest.raises(ValueError, match="truncated operands"):
        program_from_bytecode(bytes([0x11]))  # PUSH1 needs 1 operand byte
