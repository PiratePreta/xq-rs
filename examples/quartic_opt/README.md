# Quartic Optimization

Minimise a degree-4 pseudo-Boolean objective via two-stage REDUCE chaining.
Each quartic term c*x_i*x_j*x_k*x_l is converted to a quadratic QUBO term
by applying REDUCE twice in sequence.

## QUBO formulation

Decision variables x_v in {0,1}.

Objective: sum(c_t * x_i * x_j * x_k * x_l) - sum(x_v)

The linear bias -1 per variable rewards selection, creating tension with the
positive quartic terms.

Each quartic term (i, j, k, l, c) is encoded via two-stage REDUCE:

1. w = REDUCE(i, j, P_AUX)
   Introduces auxiliary w with Rosenberg enforcement; w approximates x_i*x_j.

2. v = REDUCE(w, k, P_AUX)
   Introduces auxiliary v with Rosenberg enforcement; v approximates w*x_k =
   x_i*x_j*x_k.  Here w is the variable INDEX returned from the first REDUCE,
   used directly as var_a for the second REDUCE.

3. ADDQUAD(v, l, c)
   Adds c*v*x_l = c*x_i*x_j*x_k*x_l to the QUBO.

Each quartic term allocates 2 auxiliary variables.  With M terms, the model
grows by 2*M variables beyond the original N.

## DSL methods used

- `model.reduce(var_a, var_b, p_aux)` -- two chained HOBO degree reductions;
  the RegLoad returned by the first REDUCE is passed as var_a to the second

## Usage

```sh
uv run python examples/quartic_opt/runner.py --seed 42
uv run python examples/quartic_opt/runner.py --n 6 --m 3 --interpreter rust
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--n` | 5 | Number of variables |
| `--m` | 2 | Number of quartic terms |
| `--seed` | 42 | Random seed for term generation |
| `--interpreter` | python | XQVM backend: `python` or `rust` |
| `-o PATH` | stdout | Write JSON result to file |
