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
Max-Cut end-to-end XQuad pipeline example.

Build a random weighted complete graph, compile it to XQVM assembly
via xqcp, run the encoder on the chosen VM (Python reference or
Rust), sample the resulting QUBO with xqsa's neal backend, run the
verifier and decoder on the sampled solution, and print the decoded
partition.

The runner is the showcase: a single file exercises every layer of
the toolchain. No pre-authored .xqasm files, no JSON inputs — just
CP in, decoded partition out.

Usage:
    uv run python examples/maxcut/runner.py --seed 42
    uv run python examples/maxcut/runner.py --n 6 --interpreter rust
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
from xqcp import Problem, Types
from xqsa import NealBackend
from xqvm_py import XQMX, Executor, Vec, XQMXDomain, XQMXMode, program_from_xqasm


def build_problem(n: int, seed: int) -> tuple[Problem, list[tuple[int, int, int]]]:
    """Construct a Max-Cut problem with a random weighted complete graph.

    Edges are `(i, j, w)` triples with `i < j`, fed to the VM as a flat
    Vec of 3*|E| integers.
    """
    rng = random.Random(seed)
    edges: list[tuple[int, int, int]] = [(i, j, rng.randint(1, 100)) for i in range(n) for j in range(i + 1, n)]

    problem = Problem("MaxCut")

    num_nodes = problem.input("num_nodes", type=Types.Int)
    edges_in = problem.input("edges", type=Types.Vec)

    problem.define_model(size=num_nodes, domain=XQMXDomain.BINARY)

    edge_count = problem.stow("edge_count", edges_in.veclen() // 3)
    with problem.range(0, edge_count) as e:
        offset = e * 3
        i = problem.stow("i", edges_in.get(offset))
        j = problem.stow("j", edges_in.get(offset + 1))
        w = problem.stow("w", edges_in.get(offset + 2))
        problem.model.linear[i].add(-w)
        problem.model.linear[j].add(-w)
        problem.model.quadratic[i, j].add(w * 2)

    partition = problem.output("partition", type=Types.Vec)
    with problem.range(0, num_nodes) as node:
        partition.append(problem.sample.getline(node))

    return problem, edges


def cut_weight(partition: list[int], edges: list[tuple[int, int, int]]) -> int:
    """Sum the weight of edges crossing the 0/1 partition."""
    return sum(w for i, j, w in edges if partition[i] != partition[j])


def canonicalize_partition(partition: list[int]) -> list[int]:
    """Flip the partition so node 0 sits on side 0. Max-Cut is
    invariant under global bit-flip, so this normalisation makes
    output stable across interpreters without changing the cut."""
    if not partition:
        return partition
    return partition if partition[0] == 0 else [1 - b for b in partition]


def flatten_edges(edges: list[tuple[int, int, int]]) -> list[int]:
    out: list[int] = []
    for i, j, w in edges:
        out.extend((i, j, w))
    return out


def run_python(programs: Any, n: int, edges: list[tuple[int, int, int]], seed: int) -> tuple[int, int, list[int]]:
    """Full pipeline on the pure-Python reference VM."""
    edges_vec = Vec.from_list(flatten_edges(edges))

    ex = Executor()
    ex.execute(program_from_xqasm(programs.encoder), {0: n, 1: edges_vec})
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
    part_vec = ex.state.output[0]
    partition = [part_vec.get(i) for i in range(n)]

    return energy, valid, partition


def _rust_model_to_xqvm_py(rust_model: RustModel) -> XQMX:
    py_model = XQMX(
        mode=XQMXMode.MODEL,
        domain=XQMXDomain.BINARY,
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
    values = [sample.linear.get(i, 0) for i in range(sample.size)]
    return RustSample(domain="binary", values=values, rows=sample.rows, cols=sample.cols)


def run_rust(programs: Any, n: int, edges: list[tuple[int, int, int]], seed: int) -> tuple[int, int, list[int]]:
    """Full pipeline on the Rust pyo3-bound VM.

    SA runs on xqvm_py types (its only supported surface); the
    Rust path converts at the SA boundary.
    """
    flat = flatten_edges(edges)

    vm = RustVm()
    vm.set_calldata([n, flat])
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
    partition = list(vm.outputs()[0])

    return energy, valid, partition


def main() -> int:
    parser = argparse.ArgumentParser(description="Max-Cut end-to-end XQuad pipeline example")
    parser.add_argument("--n", type=int, default=5, help="Number of nodes (default: 5)")
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

    problem, edges = build_problem(args.n, args.seed)
    programs = problem.compile()

    run = run_python if args.interpreter == "python" else run_rust
    energy, valid, partition = run(programs, args.n, edges, args.seed)
    partition = canonicalize_partition(partition)

    result = {
        "_seed": args.seed,
        "_note": "canonical CI golden",
        "n": args.n,
        "partition": partition,
        "cut_weight": cut_weight(partition, edges),
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
