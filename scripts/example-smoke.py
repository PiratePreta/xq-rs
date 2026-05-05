#!/usr/bin/env python3
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
