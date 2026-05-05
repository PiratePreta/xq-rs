# Max Independent Set

Find the largest subset of nodes in an undirected graph such that no two
selected nodes share an edge.

## QUBO formulation

Decision variables x_i in {0,1} (x_i = 1 if node i is in the independent set).

Objective: maximise sum(x_i)  =>  minimise -sum(x_i)

Constraint per edge (i,j): x_i + x_j <= 1

Each edge inequality is encoded via SLACK + EQUALITY. A single binary slack
variable s (capacity = 1) converts x_i + x_j <= 1 into the equality
x_i + x_j + s = 1, and EQUALITY adds the penalty P*(x_i + x_j + s - 1)^2.

Slack variable indices start at num_nodes and are allocated one per edge.

## DSL methods used

- `problem.vec()` -- allocate untyped vector registers for indices and coefficients
- `problem.slack(indices, coeffs, start_index, capacity)` -- append one slack entry per edge
- `model.apply_equality(indices, coeffs, target, penalty)` -- EQUALITY constraint

## Usage

```sh
uv run python examples/max_independent_set/runner.py --seed 42
uv run python examples/max_independent_set/runner.py --n 7 --interpreter rust
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--n` | 5 | Number of nodes |
| `--seed` | 42 | Random seed for graph generation |
| `--interpreter` | python | XQVM backend: `python` or `rust` |
| `-o PATH` | stdout | Write JSON result to file |
