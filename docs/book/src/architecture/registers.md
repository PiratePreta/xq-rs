# Register File

The register file is a fixed array of 256 slots, indexed `r0` through `r255`.
Each slot holds a typed `RegVal` value.

## Properties

| Property | Value |
|----------|-------|
| Count | 256 (r0--r255) |
| Index type | `u8` |
| Value type | `RegVal` (polymorphic enum) |
| Default value | `Int(0)` for all slots |

## RegVal Variants

| Variant | Rust Type | Description |
|---------|-----------|-------------|
| `Int(i64)` | `i64` | Default. Exchanged with the stack via `LOAD`/`STOW`. |
| `VecInt(Vec<i64>)` | `Vec<i64>` | Integer vector. Created by `VEC`/`VECI`. |
| `VecXqmx(Vec<XqmxModel>)` | `Vec<XqmxModel>` | Vector of models. Created by `VECX`. |
| `Model(XqmxModel)` | struct | QUBO/Ising/discrete Hamiltonian. Created by `BQMX`/`SQMX`/`XQMX`. |
| `Sample(XqmxSample)` | struct | Variable-assignment vector. Created by `BSMX`/`SSMX`/`XSMX`. |

## Type Checking

Register access is type-checked at runtime. Instructions that expect a
specific variant (e.g. `LOAD` expects `Int`, `VECPUSH` expects `VecInt`,
`SETLINE` expects `Model`) will produce a `RegisterType` error if the register
holds a different variant. The error message includes the expected and actual
type names.

## XqmxModel Structure

A model represents a QUBO/Ising/discrete Hamiltonian:

```
XqmxModel {
    domain: Domain,                      // Binary | Spin | Discrete(k)
    size: usize,                         // number of variables
    linear: BTreeMap<usize, i64>,        // bias terms h_i
    quadratic: BTreeMap<(usize,usize), i64>,  // coupling terms J_{ij}
    rows: usize,                         // grid rows (set by RESIZE)
    cols: usize,                         // grid cols (set by RESIZE)
}
```

Coefficients are stored sparsely. Missing entries read as `0`; setting a
coefficient to `0` removes it from the map.

## XqmxSample Structure

A sample holds a vector of variable assignments:

```
XqmxSample {
    domain: Domain,        // must match the model's domain
    values: Vec<i64>,      // one value per variable
}
```

## Memory Management

There is no garbage collector. Registers hold their values until explicitly
overwritten. Use `DROP reg` to reset a register to `Int(0)`, releasing any
heap allocation (models, vectors, samples) it held.
