<!--
Copyright (C) 2026 Postquant Labs Incorporated
SPDX-License-Identifier: AGPL-3.0-or-later
-->

# XQVM

> **Work in progress.** The instruction set, binary format, and public API are
> unstable and will change without notice until a stable release is tagged.
> Production use is not recommended at this time.

Aglais is a hardware-agnostic virtual machine for quantum computing. XQVM is
the module within Aglais responsible for expressing binary optimization models
and objective functions. The current scope targets X-quadratic models for
quantum annealers (QUBO/Ising formulations).

Think of it as LLVM for quantum computing.

## Goals

- **Hardware-agnostic** -- write a problem once, run it on any supported backend.
- **Unified bytecode** -- a common intermediate representation for binary
  optimization problems targeting quantum annealers.
- **Embeddable** -- the core VM and bytecode crates support `no_std + alloc`,
  enabling deployment in WASM runtimes and bare-metal environments.

## Workspace Crates

| Crate | Binary | Description |
|---|---|---|
| `aglais-xqvm-bytecode` | -- | Opcode table, instruction types, builder, binary codec, stream reader |
| `aglais-xqvm-asm` | -- | Text assembler: `.xqasm` source -> bytecode |
| `aglais-xqvm-disasm` | -- | Bytecode -> human-readable listing |
| `aglais-xqvm-vm` | -- | Bytecode interpreter: stack, register file, QUBO/Ising model execution |
| `aglais-xqvm-cli` | `xq` | Unified CLI driver (`xq asm`, `xq dism`, `xq run`) |

## Getting Started

### Prerequisites

```sh
# Install Rust (stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install dev tools
make deps
```

### Build

```sh
cargo build --release
```

The binary is placed at `target/release/xq`.

### Python workflow (REPL + examples)

Once per environment, run the baseline local-setup target:

```sh
make xquad
```

This syncs the Python workspace (`xqvm_py`, `xqcp`, `xqsa`, `xqapi`) into
`.venv/`, builds the `xqapi_py` pyo3 extension via maturin, writes a workspace
`.pth` so scripts anywhere in the repo can `import xqcp` / `xqsa` / `xqvm_py`
naturally, and installs the `xquad` CLI binary under `~/.cargo/bin/`.

Open a Python REPL with all workspace packages ready:

```sh
make repl
```

Inside the REPL:

```python
from xqapi_py.vm import Vm, XqmxModel, XqmxSample
from xqapi_py.asm import parse_xqasm, assemble_source, disassemble
from xqcp import Problem, Types
from xqsa import NealBackend
```

Run the bundled examples (TSP, Max-Cut) end-to-end. Each runner compiles the
problem via `xqcp`, runs encoder → SA (via `xqsa.NealBackend`) → verifier →
decoder on the chosen VM, and prints the decoded solution.

```sh
# Single run, stdout output:
uv run --no-sync python examples/tsp/runner.py --seed 42
uv run --no-sync python examples/maxcut/runner.py --seed 42 --interpreter rust

# Run every example on both interpreters, diff against golden.json:
make example-smoke
```

See [examples/tsp/README.md](examples/tsp/README.md) and
[examples/maxcut/README.md](examples/maxcut/README.md) for per-example details.

### Run an example

```sh
# Assemble a source file
xquad asm program.xqasm -o program.xqbc

# Disassemble to inspect the encoding
xquad dism program.xqbc

# Execute
xquad run program.xqbc
```

### A minimal program

```asm
; push two integers and add them
PUSH 10
PUSH 32
ADD
HALT
```

Assemble and run:

```sh
xquad asm add.xqasm -o add.xqbc && xquad run add.xqbc
```

## Architecture

Aglais is a stack-based interpreter with a 256-slot register file. XQVM is the
module that handles X-quadratic model construction: registers hold typed values
-- integers, integer vectors, QUBO/Ising models (`XqmxModel`), and candidate
solutions (`XqmxSample`). A dedicated loop stack drives `RANGE`/`ITER`
iteration.

The opcode table (`opcodes!` x-macro in `crates/bytecode/src/types/table.rs`) is
the single source of truth for all 76 instructions. The `Opcode` enum, `Instruction`
enum, mnemonic strings, and operand arity are all derived from it.

The binary format is a bare instruction stream with no header. Each instruction is
an opcode byte followed by its operands in big-endian byte order.

See [`examples/tsp/`](examples/tsp/) for a complete end-to-end worked example:
a Travelling Salesman Problem driven from xqcp DSL through the VM and solver,
runnable on either the Python reference VM or the Rust VM. The Rust-native
`cargo run --example tsp` showcase still lives at [`xqvm/examples/tsp/`](xqvm/examples/tsp/).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow, commit
conventions, and sign-off requirements.

```sh
make all   # lint + test (what CI runs)
```

## License

Licensed under the [GNU Affero General Public License v3.0 or later](LICENSE).
