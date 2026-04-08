# Examples

This section contains worked examples demonstrating XQVM in practice.

## Available Examples

- [Travelling Salesman Problem](tsp.md) -- a complete three-program pipeline
  that formulates, verifies, and decodes a TSP as a QUBO.

## Running Examples

The TSP example is included in the repository as a Rust integration test:

```sh
cargo run --example tsp -p aglais-xqvm-vm
```
