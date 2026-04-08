# VM Architecture

XQVM is a stack-based bytecode interpreter. A running VM holds four pieces of
mutable state:

```mermaid
block-beta
  columns 2
  block:header:2
    columns 1
    title["XQVM State"]
  end
  A["Stack"] B["Vec‹i64›, max 8192 items (LIFO operand stack)"]
  C["Register File"] D["[RegVal; 256], indexed r0–r255"]
  E["Loop Stack"] F["Vec‹LoopFrame› for RANGE/ITER iteration"]
  G["Calldata / Outputs"] H["Vec‹RegVal› — read-only inputs (INPUT) and writable output slots (OUTPUT)"]

  style title fill:none,stroke:none
  style A text-align:left
  style C text-align:left
  style E text-align:left
  style G text-align:left
```

## Design Principles

- **Stack-based computation** -- arithmetic and comparisons operate on an
  integer stack. This keeps the instruction set simple and compact.
- **Typed register file** -- registers hold polymorphic `RegVal` values (integers,
  vectors, models, samples). Type checking happens at runtime.
- **No heap / no pointers** -- there is no explicit memory allocation. Vectors
  and models grow dynamically within registers. Programs cannot address raw
  memory.
- **Deterministic execution** -- given the same program, calldata, and
  configuration, the VM always produces the same output. There are no
  random instructions or non-deterministic operations.
- **Embeddable** -- the VM crate supports `no_std + alloc`, enabling deployment
  in WASM runtimes and bare-metal environments.

## Chapters

- [Operand Stack](stack.md) -- the `i64` value stack
- [Register File](registers.md) -- the 256-slot typed register array
- [Loop Stack](loop-stack.md) -- range and iterator loop frames
- [Calldata and Outputs](calldata-outputs.md) -- external I/O slots
- [Execution Model](execution-model.md) -- the fetch-decode-execute cycle
