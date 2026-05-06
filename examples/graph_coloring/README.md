# Graph Coloring

Assign one of C colors to each node of an undirected graph such that no two
adjacent nodes share the same color (proper C-coloring).

## QUBO formulation

Decision variables x[v,c] in {0,1} (x[v,c] = 1 if node v gets color c).
The model is a 2D grid of size N*C with N rows (nodes) and C columns (colors).

One-hot constraint per node: sum_c x[v,c] = 1

Exclusion constraint per (edge, color): x[u,c] + x[v,c] <= 1
    -- adjacent nodes cannot both have color c

Both constraints are hard (penalty 200).  Any valid C-coloring achieves
energy 0 in the QUBO.  If the graph is not C-colorable, the minimum energy
solution represents the best feasible partial coloring.

## Encoding strategy

ONEHOTR applies the one-hot row constraint directly: for each node v, a
single ONEHOTR instruction constrains all C variables in that row to sum to 1.

EXCLUDE is applied per (edge (u,v), color c) pair via a nested range loop.
The 2D coordinates (u, c) and (v, c) are resolved to flat indices using
IDXGRID: `u * num_colors + c` and `v * num_colors + c`.

## DSL methods used

- `problem.define_model(size=N*C, rows=N, cols=C)` -- 2D grid model layout
- `model.apply_onehot_row(node, penalty)` -- ONEHOTR per node
- `model.apply_exclude((u, c), (v, c), penalty)` -- EXCLUDE per edge per color

## Usage

```sh
uv run python examples/graph_coloring/runner.py --seed 42
uv run python examples/graph_coloring/runner.py --n 6 --colors 3 --interpreter rust
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--n` | 5 | Number of nodes |
| `--colors` | 3 | Number of colors |
| `--seed` | 42 | Random seed for graph generation |
| `--interpreter` | python | XQVM backend: `python` or `rust` |
| `-o PATH` | stdout | Write JSON result to file |
