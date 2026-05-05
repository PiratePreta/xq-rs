# Bin Packing

Pack N items with given integer sizes into the minimum number of bins, each
with a fixed capacity C.

## QUBO formulation

Decision variables x[i,b] in {0,1} (x[i,b] = 1 if item i is placed in bin b).
The model is laid out as an N x B 2D grid of size N*B.

Objective: minimise sum_{i,b} x[i,b]  (proxy for number of bins used)

Assignment constraint per item i: sum_b x[i,b] = 1  (EQUALITY with unit coeffs)

Capacity constraint per bin b: sum_i s_i * x[i,b] <= C  (SLACK + EQUALITY)

The capacity inequality is encoded by appending binary slack variable entries
to the column index/coefficient vectors, converting it to a weighted equality.

## DSL methods used

- `problem.vec()` -- allocate untyped vector registers for indices and coefficients
- `problem.slack(indices, coeffs, start_index, capacity)` -- append slack entries
- `model.apply_equality(indices, coeffs, target, penalty)` -- EQUALITY constraint

## Usage

```sh
uv run python examples/bin_packing/runner.py --seed 42
uv run python examples/bin_packing/runner.py --n 5 --bins 4 --interpreter rust
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--n` | 4 | Number of items |
| `--bins` | 3 | Number of bins |
| `--seed` | 42 | Random seed for item generation |
| `--interpreter` | python | XQVM backend: `python` or `rust` |
| `-o PATH` | stdout | Write JSON result to file |
