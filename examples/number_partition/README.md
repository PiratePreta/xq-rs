# Number Partitioning

Given N positive integers, find a way to split them into two subsets of
equal sum (or as close as possible if an exact split does not exist).

## QUBO formulation

Decision variables x_i in {0,1} (x_i = 1 puts number a_i in subset A).

Objective: minimise P * (sum(a_i * x_i) - S/2)^2

where S = sum(a_i) is the total sum.

An exact partition exists when S is even and the penalty evaluates to zero.
The QUBO minimiser finds the balanced partition when one exists, or the
most balanced split when the total is odd.

## DSL methods used

- `problem.vec()` -- allocate untyped vector registers for indices and coefficients
- `model.apply_equality(indices, coeffs, target, penalty)` -- EQUALITY constraint

## Usage

```sh
uv run python examples/number_partition/runner.py --seed 42
uv run python examples/number_partition/runner.py --n 8 --interpreter rust
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--n` | 6 | Number of integers |
| `--seed` | 42 | Random seed for number generation |
| `--interpreter` | python | XQVM backend: `python` or `rust` |
| `-o PATH` | stdout | Write JSON result to file |
