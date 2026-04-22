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
Shared pytest fixtures for XQVM test suite.
"""

import pytest

from xqvm_py.executor import Executor
from xqvm_py.opcodes import Opcode
from xqvm_py.program import Instruction, Program
from xqvm_py.state import MachineState
from xqvm_py.xqmx import XQMX


@pytest.fixture
def empty_state() -> MachineState:
    """Create a fresh MachineState with no data."""
    return MachineState()


@pytest.fixture
def binary_model() -> XQMX:
    """Create an empty binary model XQMX."""
    return XQMX.binary_model(size=10)


@pytest.fixture
def binary_sample() -> XQMX:
    """Create an empty binary sample XQMX."""
    return XQMX.binary_sample(size=10)


@pytest.fixture
def grid_model() -> XQMX:
    """Create a binary model XQMX with grid dimensions."""
    return XQMX.binary_model(size=25, rows=5, cols=5)


@pytest.fixture
def executor() -> Executor:
    """Create a fresh Executor instance."""
    return Executor()


@pytest.fixture
def simple_program() -> Program:
    """Create a simple program that pushes, adds, and stores."""
    return Program(
        [
            Instruction(Opcode.PUSH1, (10,)),
            Instruction(Opcode.PUSH1, (5,)),
            Instruction(Opcode.ADD),
            Instruction(Opcode.STOW, (0,)),
            Instruction(Opcode.HALT),
        ]
    )
