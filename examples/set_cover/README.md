# Set Cover

Given a universe of E elements and a collection of S sets, find the minimum
sub-collection whose union equals the universe.

## QUBO formulation

Decision variables x_s in {0,1} (x_s = 1 if set s is selected).

Objective: minimise sum(x_s)

Constraint per element e: sum_{s: covers[e][s]=1} x_s >= 1

The coverage membership matrix (covers[e][s] = 1 if set s covers element e)
is passed as a flat vec of size E*S at runtime.  For each element, the encoder
iterates over all sets and uses a branch to conditionally push only covering
set indices into the element's index vector.  ATLEAST then enforces that at
least one covering set is selected.

## DSL methods used

- `problem.vec()` -- allocate a vector register for each element's covering set indices
- `problem.branch(cond, arm, default)` -- conditional VECPUSH based on coverage membership
- `model.apply_atleast(indices, k, penalty)` -- ATLEAST constraint with k=1

## Usage

```sh
uv run python examples/set_cover/runner.py --seed 42
uv run python examples/set_cover/runner.py --num-sets 6 --interpreter rust
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--num-elements` | 4 | Number of elements in the universe |
| `--num-sets` | 5 | Number of sets |
| `--seed` | 42 | Random seed for coverage matrix generation |
| `--interpreter` | python | XQVM backend: `python` or `rust` |
| `-o PATH` | stdout | Write JSON result to file |
