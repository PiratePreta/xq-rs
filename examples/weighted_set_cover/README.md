# Weighted Set Cover

A generalisation of Set Cover where each set s has a coverage capacity cap[s]
and each element e has a demand demand[e].  The goal is to select sets of
minimum total cost such that the total capacity of covering selected sets
meets each element's demand.

## QUBO formulation

Decision variables x_s in {0,1} (x_s = 1 if set s is selected).

Objective: minimise sum(cost[s] * x_s)

Constraint per element e:
    sum_{s: covers[e][s]=1} cap[s] * x_s >= demand[e]

This weighted at-least-k constraint is encoded directly with ATLEASTW.
For each element, a branch conditionally pushes (set index, capacity) pairs
into per-element index/coefficient vectors, then ATLEASTW enforces the
weighted threshold.

## DSL methods used

- `problem.vec()` -- allocate vector registers for covering set indices and capacities
- `problem.branch(cond, arm, default)` -- conditional VECPUSH based on coverage membership
- `model.apply_atleastw(indices, coeffs, k, penalty)` -- ATLEASTW constraint

## Usage

```sh
uv run python examples/weighted_set_cover/runner.py --seed 42
uv run python examples/weighted_set_cover/runner.py --num-sets 6 --interpreter rust
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--num-elements` | 4 | Number of elements in the universe |
| `--num-sets` | 5 | Number of sets |
| `--seed` | 42 | Random seed for instance generation |
| `--interpreter` | python | XQVM backend: `python` or `rust` |
| `-o PATH` | stdout | Write JSON result to file |
