# Vertex Cover

Find the minimum subset of vertices such that every edge in an undirected
graph has at least one endpoint in the subset.

## QUBO formulation

Decision variables x_v in {0,1} (x_v = 1 if vertex v is in the cover).

Objective: minimise sum(x_v)

Constraint per edge (i,j): x_i + x_j >= 1

The at-least-1 constraint is encoded directly with ATLEAST. For each edge,
ATLEAST allocates one slack variable (S = floor(log2(2-1)) + 1 = 1) at
model.size and adds the penalty P*(x_i + x_j - 1 - s)^2, where s in {0,1}
accounts for the case when both endpoints are selected (sum = 2).

## DSL methods used

- `problem.vec()` -- allocate a vector register for the two endpoint indices per edge
- `model.apply_atleast(indices, k, penalty)` -- ATLEAST constraint with k=1

## Usage

```sh
uv run python examples/vertex_cover/runner.py --seed 42
uv run python examples/vertex_cover/runner.py --n 7 --interpreter rust
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--n` | 5 | Number of nodes |
| `--seed` | 42 | Random seed for graph generation |
| `--interpreter` | python | XQVM backend: `python` or `rust` |
| `-o PATH` | stdout | Write JSON result to file |
