<!--
Copyright (C) 2026 Postquant Labs Incorporated
SPDX-License-Identifier: AGPL-3.0-or-later
-->

# Aglais XQVM

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
| `aglais-xqvm-asm` | `xqasm` | Text assembler: `.xqasm` source -> bytecode |
| `aglais-xqvm-disasm` | `xqdism` | Bytecode -> human-readable listing |
| `aglais-xqvm-vm` | `xqvm` | Bytecode interpreter: stack, register file, QUBO/Ising model execution |

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

The binaries are placed at `target/release/xqasm`, `target/release/xqdism`,
and `target/release/xqvm`.

### Run an example

```sh
# Assemble a source file
xqasm program.xqasm -o program.xqbc

# Disassemble to inspect the encoding
xqdism program.xqbc

# Execute
xqvm program.xqbc
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
xqasm add.xqasm -o add.xqbc && xqvm add.xqbc
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

See `crates/vm/examples/tsp/` for a complete worked example: a Travelling Salesman
Problem encoded as a QUBO via three `.xqasm` programs driven by a Rust harness.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow, commit
conventions, and sign-off requirements.

```sh
make all   # lint + test (what CI runs)
```

## License

Licensed under the [GNU Affero General Public License v3.0 or later](LICENSE).
