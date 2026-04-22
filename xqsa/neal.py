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
DWave neal simulated annealing backend.

Wraps dwave-neal's SimulatedAnnealingSampler to solve XQMX
quadratic models via simulated annealing on CPU.
"""

from __future__ import annotations

import time
from typing import Any

import dimod
import neal

from xqvm_py.xqmx import XQMX, XQMXDomain

from .backend import Backend, SolverResult


class NealBackend(Backend):
    """Simulated annealing backend using dwave-neal."""

    def __init__(
        self,
        num_reads: int = 100,
        num_sweeps: int = 1000,
        beta_range: tuple[float, float] | None = None,
        seed: int | None = None,
    ) -> None:
        self.num_reads = num_reads
        self.num_sweeps = num_sweeps
        self.beta_range = beta_range
        self.seed = seed

    def solve(self, model: XQMX, **kwargs: Any) -> SolverResult:
        """Solve using simulated annealing via dwave-neal."""
        self._validate_model(model)

        num_reads = kwargs.get("num_reads", self.num_reads)
        num_sweeps = kwargs.get("num_sweeps", self.num_sweeps)
        beta_range = kwargs.get("beta_range", self.beta_range)
        seed = kwargs.get("seed", self.seed)

        bqm = self._model_to_bqm(model)
        sampler = neal.SimulatedAnnealingSampler()

        sample_kwargs: dict[str, Any] = {
            "num_reads": num_reads,
            "num_sweeps": num_sweeps,
        }
        if beta_range is not None:
            sample_kwargs["beta_range"] = beta_range
        if seed is not None:
            sample_kwargs["seed"] = seed

        t0 = time.perf_counter()
        result = sampler.sample(bqm, **sample_kwargs)
        elapsed = time.perf_counter() - t0

        best = result.first
        raw_sample = dict(best.sample)
        energy = float(best.energy)

        sample = self._sample_to_xqmx(model, raw_sample)

        return SolverResult(
            sample=sample,
            energy=energy,
            timing=elapsed,
            metadata={
                "num_reads": num_reads,
                "num_sweeps": num_sweeps,
                "beta_range": beta_range,
                "seed": seed,
                "num_occurrences": int(best.num_occurrences),
            },
        )

    def _model_to_bqm(self, model: XQMX) -> dimod.BinaryQuadraticModel:
        """Convert an XQMX model to a dimod BQM."""
        vartype = dimod.BINARY if model.domain == XQMXDomain.BINARY else dimod.SPIN
        return dimod.BinaryQuadraticModel(
            model.linear,
            model.quadratic,
            0.0,
            vartype,
        )

    def _sample_to_xqmx(self, model: XQMX, raw_sample: dict[int, int]) -> XQMX:
        """Convert a dimod sample dict to an XQMX sample."""
        if model.domain == XQMXDomain.BINARY:
            sample = XQMX.binary_sample(model.size, model.rows, model.cols)
        else:
            sample = XQMX.spin_sample(model.size, model.rows, model.cols)

        for var_idx, value in raw_sample.items():
            sample.set_linear(var_idx, int(value))

        return sample
