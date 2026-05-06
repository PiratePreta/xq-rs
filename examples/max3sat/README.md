# Max-3-SAT

Given M clauses of 3 positive literals over N binary variables, find the
assignment that satisfies the maximum number of clauses.

## QUBO formulation

Decision variables x_v in {0,1}.

Objective: minimise sum over clauses of P*(1-x_i)(1-x_j)(1-x_k)

A clause (i,j,k) is violated when all three variables are 0.  Expanding
the product (dropping the constant term):

    P*(-x_i - x_j - x_k + x_i*x_j + x_i*x_k + x_j*x_k - x_i*x_j*x_k)

The cubic term -P*x_i*x_j*x_k is degree-reduced via REDUCE(i, j) -> w,
introducing one auxiliary variable w per clause with Rosenberg enforcement
P_AUX*(x_i*x_j - 2*x_i*w - 2*x_j*w + 3*w).  The cubic term becomes the
quadratic term -P*w*x_k.

## DSL methods used

- `model.reduce(var_a, var_b, p_aux)` -- HOBO degree reduction; returns a
  RegLoad holding the auxiliary variable index for chaining into quadratic terms

## Usage

```sh
uv run python examples/max3sat/runner.py --seed 42
uv run python examples/max3sat/runner.py --n 8 --m 10 --interpreter rust
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--n` | 6 | Number of Boolean variables |
| `--m` | 8 | Number of clauses |
| `--seed` | 42 | Random seed for clause generation |
| `--interpreter` | python | XQVM backend: `python` or `rust` |
| `-o PATH` | stdout | Write JSON result to file |
