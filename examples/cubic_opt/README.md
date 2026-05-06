# Cubic Optimization

Minimise a cubic pseudo-Boolean objective over binary variables.  Each degree-3
interaction term c*x_i*x_j*x_k is reduced to a quadratic QUBO term via a
single REDUCE call (Higher-Order Binary Optimisation -- HOBO degree reduction).

## QUBO formulation

Decision variables x_v in {0,1}.

Objective: sum(c_t * x_i * x_j * x_k) - sum(x_v)

The linear bias -1 per variable rewards selection, creating tension with
the positive cubic terms so the optimal solution is non-trivial.

Each cubic term (i, j, k, c) is encoded as:
1. REDUCE(i, j, P_AUX) -> w  -- allocates auxiliary variable w with
   Rosenberg enforcement P_AUX*(x_i*x_j - 2*x_i*w - 2*x_j*w + 3*w)
2. ADDQUAD(w, k, c) -- adds c*w*x_k = c*x_i*x_j*x_k to the QUBO

## DSL methods used

- `model.reduce(var_a, var_b, p_aux)` -- single-stage HOBO degree reduction

## Usage

```sh
uv run python examples/cubic_opt/runner.py --seed 42
uv run python examples/cubic_opt/runner.py --n 5 --m 4 --interpreter rust
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--n` | 4 | Number of variables |
| `--m` | 3 | Number of cubic terms |
| `--seed` | 42 | Random seed for term generation |
| `--interpreter` | python | XQVM backend: `python` or `rust` |
| `-o PATH` | stdout | Write JSON result to file |
