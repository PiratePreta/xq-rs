"""Command-line interface for the XQVM reference implementation.

Thin shim around the in-process Executor API so that xqvm-py can be
driven with the same I/O contract as the Rust `xquad run` binary. Used
by the xquad-conformance harness to validate identical outputs across
implementations.

Invocation:

    python -m xqvm run [--text] [--inputs inputs.json | --calldata 1,2,3]
                      [--outputs N] PROGRAM
"""

from .run import main

__all__ = ["main"]
