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
TSP end-to-end XQuad pipeline example.

Build a random Travelling-Salesman-Problem instance, compile it to
XQVM assembly via xqcp, run the encoder on the chosen VM (Python
reference or Rust), sample the resulting QUBO with xqsa's neal
backend, run the verifier and decoder on the sampled solution, and
print the decoded tour.

The runner is the showcase: a single file exercises every layer of
the toolchain. No pre-authored .xqasm files, no JSON inputs — just
CP in, decoded tour out.

Usage:
    uv run python examples/tsp/runner.py --seed 42
    uv run python examples/tsp/runner.py --n 5 --interpreter rust
"""

from __future__ import annotations

import argparse
import json
import random
import sys
from pathlib import Path
from typing import Any

from xqapi_py.asm import assemble_source
from xqapi_py.vm import Vm as RustVm
from xqapi_py.vm import XqmxModel as RustModel
from xqapi_py.vm import XqmxSample as RustSample
from xqcp import Problem, Types, xq_triu
from xqsa import NealBackend
from xqvm_py import XQMX, Executor, Vec, XQMXDomain, XQMXMode, program_from_xqasm, triu


def build_problem(n: int, seed: int) -> tuple[Problem, list[int]]:
    """Construct a TSP problem with a random symmetric distance matrix.

    Distances live in the upper triangle only (`n*(n-1)/2` entries)
    and are fed to the VM as a flat Vec indexed by xq_triu(i, j).
    """
    rng = random.Random(seed)
    distances = [rng.randint(1, 100) for _ in range(n * (n - 1) // 2)]

    problem = Problem("TSP")

    num_cities = problem.input("num_cities", type=Types.Int)
    distance_matrix = problem.input("distance_matrix", type=Types.Vec)

    problem.define_model(
        size=num_cities * num_cities,
        domain=XQMXDomain.BINARY,
        rows=num_cities,
        cols=num_cities,
    )

    with problem.range(0, num_cities - 1) as city_i:
        with problem.range(city_i + 1, num_cities) as city_j:
            dist = problem.stow("dist", distance_matrix.get(xq_triu(city_i, city_j)))
            with problem.range(0, num_cities) as position:
                next_position = (position + 1) % num_cities
                problem.model.quadratic[(city_i, position), (city_j, next_position)].add(dist)
                problem.model.quadratic[(city_j, position), (city_i, next_position)].add(dist)

    with problem.range(0, num_cities) as city:
        problem.model.apply_onehot_row(city, penalty=100)
    with problem.range(0, num_cities) as position:
        problem.model.apply_onehot_col(position, penalty=100)

    tour = problem.output("tour", type=Types.Vec)
    with problem.range(0, num_cities) as position:
        tour.append(problem.sample.colfind(col=position, value=1))

    return problem, distances


def tour_distance(tour: list[int], distances: list[int], n: int) -> int:
    """Sum the cost of a tour (distances indexed by upper-triangle triu)."""
    total = 0
    for p in range(n):
        i, j = tour[p], tour[(p + 1) % n]
        if i == j:
            continue
        total += distances[triu(i, j)]
    return total


def canonicalize_tour(tour: list[int]) -> list[int]:
    """Rotate the tour so city 0 leads. TSP tours are cyclic, so the
    rotation has no semantic effect but makes the output stable across
    interpreters (SA's variable-ordering heuristics can emit rotations
    of the same cycle depending on how the BQM was built)."""
    if 0 not in tour:
        return tour
    idx = tour.index(0)
    return tour[idx:] + tour[:idx]


def run_python(programs: Any, n: int, distances: list[int], seed: int) -> tuple[int, int, list[int]]:
    """Full pipeline on the pure-Python reference VM."""
    dist_vec = Vec.from_list(distances)

    ex = Executor()
    ex.execute(program_from_xqasm(programs.encoder), {0: n, 1: dist_vec})
    model = ex.state.output[0]
    assert isinstance(model, XQMX)

    backend = NealBackend(seed=seed)
    sample = backend.solve(model).sample

    ex = Executor()
    ex.execute(program_from_xqasm(programs.verifier), {0: model, 1: sample, 2: n})
    energy = ex.state.output[0]
    valid = ex.state.output[1]

    ex = Executor()
    ex.execute(program_from_xqasm(programs.decoder), {0: sample, 1: n})
    tour_vec = ex.state.output[0]
    tour = [tour_vec.get(i) for i in range(n)]

    return energy, valid, tour


def _rust_model_to_xqvm_py(rust_model: RustModel) -> XQMX:
    """Convert a xqapi_py.XqmxModel into an xqvm_py.XQMX so xqsa can sample it."""
    py_model = XQMX(
        mode=XQMXMode.MODEL,
        domain=XQMXDomain.BINARY,  # TSP models are binary
        size=rust_model.size,
        rows=rust_model.rows,
        cols=rust_model.cols,
    )
    for idx, coeff in rust_model.linear_items():
        py_model.linear[idx] = coeff
    for (i, j), coeff in rust_model.quadratic_items():
        py_model.quadratic[i, j] = coeff
    return py_model


def _xqvm_py_sample_to_rust(sample: XQMX) -> RustSample:
    """Convert an xqvm_py sample into an xqapi_py.XqmxSample for the Rust VM."""
    values = [sample.linear.get(i, 0) for i in range(sample.size)]
    return RustSample(domain="binary", values=values, rows=sample.rows, cols=sample.cols)


def run_rust(programs: Any, n: int, distances: list[int], seed: int) -> tuple[int, int, list[int]]:
    """Full pipeline on the Rust pyo3-bound VM.

    SA is always driven by xqvm_py types (its only supported surface),
    so the Rust path converts `XqmxModel` / `XqmxSample` at the SA
    boundary.
    """
    vm = RustVm()
    vm.set_calldata([n, distances])
    vm.set_output_slots(1)
    vm.run(assemble_source(programs.encoder))
    rust_model = vm.outputs()[0]
    assert isinstance(rust_model, RustModel)

    backend = NealBackend(seed=seed)
    py_sample = backend.solve(_rust_model_to_xqvm_py(rust_model)).sample
    rust_sample = _xqvm_py_sample_to_rust(py_sample)

    vm = RustVm()
    vm.set_calldata([rust_model, rust_sample, n])
    vm.set_output_slots(2)
    vm.run(assemble_source(programs.verifier))
    outs = vm.outputs()
    energy, valid = outs[0], outs[1]

    vm = RustVm()
    vm.set_calldata([rust_sample, n])
    vm.set_output_slots(1)
    vm.run(assemble_source(programs.decoder))
    tour = list(vm.outputs()[0])

    return energy, valid, tour


def main() -> int:
    parser = argparse.ArgumentParser(description="TSP end-to-end XQuad pipeline example")
    parser.add_argument("--n", type=int, default=4, help="Number of cities (default: 4)")
    parser.add_argument("--seed", type=int, default=42, help="Random seed (default: 42)")
    parser.add_argument(
        "--interpreter",
        choices=("python", "rust"),
        default="python",
        help="XQVM interpreter to run the compiled programs on",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=None,
        help="Write the decoded result as JSON to this path; stdout otherwise",
    )
    args = parser.parse_args()

    problem, distances = build_problem(args.n, args.seed)
    programs = problem.compile()

    run = run_python if args.interpreter == "python" else run_rust
    energy, valid, tour = run(programs, args.n, distances, args.seed)
    tour = canonicalize_tour(tour)

    result = {
        "_seed": args.seed,
        "_note": "canonical CI golden",
        "n": args.n,
        "tour": tour,
        "tour_distance": tour_distance(tour, distances, args.n),
        "energy": int(energy),
        "valid": int(valid),
    }

    body = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output is None:
        sys.stdout.write(body)
    else:
        args.output.write_text(body)

    return 0


if __name__ == "__main__":
    sys.exit(main())
