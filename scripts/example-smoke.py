#!/usr/bin/env python3
# Copyright (C) 2026 Postquant Labs Incorporated
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU Affero General Public License as published
# by the Free Software Foundation, either version 3 of the License, or
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
Invariant-based smoke test for example programs.

Runs each example on both the Python and Rust XQVM interpreters and
verifies that both produce a valid solution (valid == 1).  Does NOT
require byte-for-byte output parity — SA is sensitive to BQM
construction order, so the two paths may find different (but equally
valid) optima.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

EXAMPLES_DIR = Path(__file__).resolve().parent.parent / "examples"
SEED = 42


def run_example(runner: Path, interpreter: str) -> dict:
    result = subprocess.run(
        ["python", str(runner), "--seed", str(SEED), "--interpreter", interpreter],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"  FAIL ({interpreter}): runner exited {result.returncode}", file=sys.stderr)
        print(result.stderr[-500:], file=sys.stderr)
        sys.exit(1)
    return json.loads(result.stdout)


def main() -> int:
    runners = sorted(EXAMPLES_DIR.glob("*/runner.py"))
    if not runners:
        print("ERROR: no examples found", file=sys.stderr)
        return 1

    failures = 0
    for runner in runners:
        name = runner.parent.name
        print(f"==> {name}")

        for interp in ("python", "rust"):
            out = run_example(runner, interp)
            valid = out.get("valid")
            energy = out.get("energy")

            if valid != 1:
                print(f"  FAIL ({interp}): valid={valid}, energy={energy}")
                failures += 1
            else:
                print(f"  ok   ({interp}): energy={energy}")

    if failures:
        print(f"\n{failures} failure(s)")
        return 1

    print(f"\nAll {len(runners)} examples passed on both interpreters.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
