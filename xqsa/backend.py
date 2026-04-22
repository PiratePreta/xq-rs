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
Abstract solver backend and result types for XQMX quadratic models.

Backends implement the solve() method to find low-energy solutions
for XQMX models using different optimization strategies.
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any

from xqvm_py.xqmx import XQMX, XQMXDomain, XQMXMode


@dataclass(frozen=True)
class SolverResult:
    """Result from a solver backend."""

    sample: XQMX
    energy: float
    timing: float
    metadata: dict[str, Any] = field(default_factory=dict)


class Backend(ABC):
    """Abstract solver backend for XQMX quadratic models."""

    @abstractmethod
    def solve(self, model: XQMX, **kwargs: Any) -> SolverResult:
        """Solve a quadratic model, returning the best solution found."""
        ...

    def _validate_model(self, model: XQMX) -> None:
        """Validate that the model is solvable."""
        if model.mode != XQMXMode.MODEL:
            raise ValueError(f"Expected MODEL mode, got {model.mode.name}")
        if model.domain not in (XQMXDomain.BINARY, XQMXDomain.SPIN):
            raise ValueError(f"Unsupported domain for solving: {model.domain.name}")
