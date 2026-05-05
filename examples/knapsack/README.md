# Knapsack

The 0/1 Knapsack problem: given N items with integer weights and values,
select a subset maximising total value subject to a weight capacity constraint.

## QUBO formulation

Decision variables x_i in {0,1} (x_i = 1 means item i is selected).

Objective: minimise -sum(v_i * x_i)

Constraint: sum(w_i * x_i) <= W

The inequality is encoded via SLACK + EQUALITY. SLACK appends binary slack
variable entries (s_j with coefficients 2^j) to the index and coefficient
vectors, converting the inequality to the equality sum(w_i*x_i) + sum(s_j*2^j) = W.
EQUALITY then adds the penalty term P*(sum(a_k*x_k) - W)^2 to the QUBO.

## DSL methods used

- `problem.vec()` -- allocate untyped vector registers for indices and coefficients
- `problem.slack(indices, coeffs, start_index, capacity)` -- append slack entries
- `model.apply_equality(indices, coeffs, target, penalty)` -- EQUALITY constraint

## Usage

```sh
uv run python examples/knapsack/runner.py --seed 42
uv run python examples/knapsack/runner.py --n 6 --interpreter rust
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--n` | 5 | Number of items |
| `--seed` | 42 | Random seed for item generation |
| `--interpreter` | python | XQVM backend: `python` or `rust` |
| `-o PATH` | stdout | Write JSON result to file |
