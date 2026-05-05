# Portfolio Optimization

Select a portfolio of exactly B assets from N candidates to maximise expected
return while penalising higher-order risk cross-interactions.

## QUBO formulation

Decision variables x_i in {0,1} (x_i = 1 if asset i is selected).

Objective: -sum(r_i * x_i) + sum(sigma_ijk * x_i * x_j * x_k)

The first term maximises return (minimising its negation).  The second term
penalises correlated three-asset risk interactions encoded as cubic terms,
reduced to quadratic via REDUCE.

Budget constraint: sum(x_i) = B  (EQUALITY with unit coefficients)

## Encoding strategy

Return terms are linear: ADDLINE(i, -r_i) per asset.

Cubic risk terms (i, j, k, sigma) are degree-reduced:
1. REDUCE(i, j, P_AUX) -> w  (Rosenberg enforcement for w = x_i * x_j)
2. ADDQUAD(w, k, sigma)      (sigma * w * x_k = sigma * x_i * x_j * x_k)

Budget constraint builds uniform-coefficient index/coeff vecs then calls
EQUALITY with target = B and penalty = 200.  EQUALITY is emitted after all
objective (body) actions because it lands in the constraint section.

## DSL methods used

- `model.reduce(var_a, var_b, p_aux)` -- HOBO degree reduction for cubic risk terms
- `problem.vec()` -- allocate index/coefficient vecs for the budget constraint
- `model.apply_equality(indices, coeffs, target, penalty)` -- budget EQUALITY

## Usage

```sh
uv run python examples/portfolio_opt/runner.py --seed 42
uv run python examples/portfolio_opt/runner.py --n 6 --budget 3 --interpreter rust
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--n` | 5 | Number of assets |
| `--budget` | 2 | Number of assets to select |
| `--seed` | 42 | Random seed for instance generation |
| `--interpreter` | python | XQVM backend: `python` or `rust` |
| `-o PATH` | stdout | Write JSON result to file |
