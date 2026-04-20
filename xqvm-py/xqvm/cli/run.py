"""Implementation of the `python -m xqvm run` subcommand."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from xqvm.assembler import assemble
from xqvm.core import Executor, Program


def _load_program(path: Path, *, text: bool) -> Program:
    """Load a .xqasm source or a .xqb bytecode file into a Program.

    `text=True` forces assembly-text mode; otherwise the extension picks
    the mode (`.xqasm` → text, anything else → binary bytecode). The
    Python reference does not yet decode `.xqb` directly; the conformance
    harness always has the `.xqasm` source adjacent to the bytecode and
    feeds that path in.
    """
    if text or path.suffix == ".xqasm":
        source = path.read_text(encoding="utf-8")
        return assemble(source, name=path.stem).program
    raise SystemExit(
        f"xqvm-py: binary bytecode ({path}) is not yet supported; "
        "pass the .xqasm source with --text or point at the .xqasm file."
    )


def _build_input_data(args: argparse.Namespace) -> dict[int, Any]:
    if args.inputs is not None:
        with Path(args.inputs).open(encoding="utf-8") as f:
            inputs_doc = json.load(f)
        calldata = inputs_doc.get("calldata", [])
    else:
        calldata = list(args.calldata)
    return {slot: value for slot, value in enumerate(calldata)}


def _serialise_value(value: Any) -> Any:
    """Convert a RegVal-ish Python value to JSON-ready primitives."""
    if isinstance(value, int):
        return value
    # Fall back to str for complex types (Vec, XQMX). This is only needed
    # for registers/outputs that hold non-scalar state; conformance
    # vectors stick to integer outputs, so this path is a defensive
    # last resort rather than a hot path.
    return repr(value)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="python -m xqvm",
        description="Run XQVM programs using the Python reference VM.",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    run = sub.add_parser("run", help="Execute an XQVM program")
    run.add_argument("file", type=Path, help="Program file (.xqasm or .xqb)")
    run.add_argument(
        "--text",
        action="store_true",
        help="Force interpretation of FILE as assembly text.",
    )
    run.add_argument(
        "--calldata",
        type=lambda s: [int(x) for x in s.split(",") if x],
        default=[],
        help="Comma-separated i64 calldata values passed to INPUT slots.",
    )
    run.add_argument(
        "--inputs",
        type=Path,
        help='JSON file with {"calldata": [...]} (overrides --calldata).',
    )
    run.add_argument(
        "--outputs",
        type=int,
        default=16,
        help="Number of output slots reserved (outputs beyond this index are null).",
    )

    args = parser.parse_args(argv)

    if args.command != "run":
        parser.error(f"unknown command: {args.command}")

    program = _load_program(args.file, text=args.text)
    input_data = _build_input_data(args)

    executor = Executor()
    output_map = executor.execute(program, input_data=input_data)

    # Unset slots are reported as 0 to match the Rust VM, whose output
    # vector is pre-filled with RegVal::default() (== Int(0)) and cannot
    # distinguish "never written" from "explicitly zero". Vectors that
    # care about the distinction should explicitly OUTPUT every slot.
    outputs: list[Any] = [
        _serialise_value(output_map[slot]) if slot in output_map else 0 for slot in range(args.outputs)
    ]
    final_stack = [_serialise_value(v) for v in executor.state.stack]

    json.dump(
        {"outputs": outputs, "final_stack": final_stack},
        sys.stdout,
        separators=(",", ":"),
    )
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
