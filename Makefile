.PHONY: all xquad repl \
        deps deps-docs deps-miri deps-python \
        install-hooks \
        lint lint-clippy lint-doc lint-deny lint-python \
        fmt fmt-rust fmt-taplo fmt-check fmt-check-rust fmt-check-taplo fmt-python fmt-check-python \
        test test-unit test-integration test-doc test-miri test-python \
        opcode-parity opcode-parity-rust opcode-parity-python \
        conformance conformance-rust conformance-python \
        example-smoke regen-example-goldens \
        docs docs-regen docs-check docs-serve

all: fmt lint test

# -- Local setup ------------------------------------------------------------

# Bootstrap everything a contributor needs to use the XQuad toolchain
# locally:
#   - Python workspace (xqvm_py, xqcp, xqsa, xqffi) synced into .venv/
#     with the maturin-built xqffi extension and the workspace .pth
#     so any script in the repo can `import xqcp` etc.
#   - Rust CLI installed as the `xquad` binary under ~/.cargo/bin/ so
#     `xquad run …`, `xquad dsm …`, etc. work from any shell.
#
# Run once per environment; re-run after a pull that touches Rust
# sources or workspace deps. Publishing / wheel distribution is out
# of scope (see QUI-442).
xquad: deps-python
	cargo install --path xqcli --locked --force

# -- Dependencies -----------------------------------------------------------

# Install (or verify) the pinned cargo-based dev tools. Versions come
# from scripts/cargo-tools.lock; the install script short-circuits
# when a tool is already on PATH at the pinned version so a warm
# local environment — or a CI cache hit — pays zero cost. First-
# install goes through cargo-binstall (prebuilt binaries in seconds)
# with a cargo-install-from-source fallback.
deps: deps-docs
	rustup component add clippy rustfmt
	bash scripts/install-cargo-tools.sh

# `deps-docs` is a no-op alias for `deps` now that mdbook / mdbook-
# mermaid are bundled into the single tool-install flow. Kept as a
# phony target so CI jobs referencing `deps-docs` don't break.
deps-docs:
	bash scripts/install-cargo-tools.sh

deps-miri:
	rustup toolchain install nightly --component miri
	cargo +nightly miri setup

# Sync the Python workspace (xqffi, xqvm_py, xqcp, xqsa) into .venv/
# via uv. Assumes `uv` is already on $PATH; CI installs it in its
# before_script.
#
# `uv sync` alone can skip rebuilding the maturin-built xqffi cdylib
# when Cargo source has changed but uv's editable-wheel cache is
# still valid, leaving .venv/.../xqffi.*.so stale. An explicit
# `maturin develop` after sync guarantees the extension matches
# current Rust sources — essential for local runs of
# `make example-smoke`, `make test-python`, etc.
#
# The final step writes `xq-rs-workspace.pth` into the venv's
# site-packages, adding the repo root to sys.path. This closes a
# flat-layout editable-install quirk: hatchling puts each package's
# own directory on sys.path (e.g. /repo/xqcp) rather than the parent
# (/repo), so scripts run from sibling directories (examples/,
# scripts/) can't `import xqcp` unless they first inject the repo
# root themselves. With the .pth in place they just work.
deps-python:
	uv sync
	uv run --active maturin develop --manifest-path xqffi/Cargo.toml
	@.venv/bin/python -c "from pathlib import Path; import site; Path(site.getsitepackages()[0], 'xq-rs-workspace.pth').write_text(str(Path('.').resolve()))"

# Point git at the repo-tracked .githooks/ directory so the pre-commit
# hook runs on every commit. Run once per clone; bypass ad hoc with
# `git commit --no-verify`. The hook only runs fast format / lint
# checks on staged files (ruff for .py, taplo for .toml); heavier
# checks stay in CI / `make all`.
install-hooks:
	git config core.hooksPath .githooks
	@echo "pre-commit hook installed. bypass with 'git commit --no-verify'."

# -- Formatting -------------------------------------------------------------

fmt: fmt-rust fmt-taplo fmt-python

fmt-rust:
	cargo fmt --all

fmt-taplo:
	taplo fmt

fmt-python:
	uv run ruff format xqvm_py xqcp xqsa xqffi xquad examples

fmt-check: fmt-check-rust fmt-check-taplo fmt-check-python

fmt-check-rust:
	cargo fmt --all -- --check

fmt-check-taplo:
	taplo fmt --check

fmt-check-python:
	uv run ruff format --check xqvm_py xqcp xqsa xqffi xquad examples

# -- Lints ------------------------------------------------------------------

lint: lint-clippy lint-doc lint-deny lint-python fmt-check

lint-clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

lint-doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

lint-deny:
	cargo deny check

lint-python:
	uv run ruff check xqvm_py xqcp xqsa xqffi xquad examples

# -- Tests ------------------------------------------------------------------

test: test-unit test-integration test-doc test-python

test-unit:
	cargo nextest run --workspace --all-features --lib --cargo-profile ci-test

# xquad-conformance is excluded here because its `python` feature gates
# a test file that shells out to `uv run python -m xqvm_py`, and the
# test:integration CI job does not install uv. The conformance suite
# has its own dedicated jobs (spec-conformance-rust / -python) that
# cover both runtimes with the proper before_script setup.
test-integration:
	cargo nextest run --workspace --exclude xquad-conformance --all-features --test '*' --cargo-profile ci-test

# nextest cannot execute rustdoc doctests, so they are driven by the
# built-in test harness on a dedicated target.
test-doc:
	cargo test --doc --workspace --all-features --profile ci-test

test-miri:
	cargo +nightly miri test --workspace --all-features

# `uv run pytest` alone skips rebuilding xqffi's maturin-built cdylib
# when Rust sources have changed (uv's editable-wheel cache masks the
# edit). Depend on deps-python so a fresh maturin develop runs first;
# CI already has this via the job's before_script.
test-python: deps-python
	uv run --no-sync pytest xqvm_py/tests xqcp/tests xqsa/tests xquad/tests

# -- Conformance ------------------------------------------------------------

# Cross-implementation parity (opcode table, spec conformance vectors).
# `cargo build -p xqvm` exercises the compile-time YAML ↔ opcodes! macro
# check via xqvm/build.rs; the Python script covers the xqvm_py side.
opcode-parity: opcode-parity-rust opcode-parity-python

opcode-parity-rust:
	cargo build -p xqvm

opcode-parity-python:
	uv run python scripts/check-opcode-parity.py

conformance: conformance-rust conformance-python

conformance-rust:
	cargo test -p xquad-conformance --no-default-features --features rust

# spec-conformance-python shells out to `uv run python -m xqvm_py run`
# from within the Rust test; the xqffi extension (maturin-built)
# and xqvm_py (editable) must both be installed in .venv/ first.
conformance-python:
	cargo test -p xquad-conformance --no-default-features --features python

# -- Dev ergonomics ---------------------------------------------------------

# Open a Python REPL with the xqffi extension fresh and the
# workspace packages (xqvm_py, xqcp, xqsa) importable. Depends on
# deps-python so the .so / .pth stay current; `uv run --no-sync`
# skips the implicit sync that would otherwise revert maturin's
# fresh extension build to a cached wheel.
repl: deps-python
	uv run --no-sync python

# -- Examples ---------------------------------------------------------------

# Run each top-level example on both the Python and the Rust XQVM
# interpreters with the canonical seed and diff the decoded outputs
# against the checked-in golden.json. Catches drift between the two
# interpreters and regressions in either path.
example-smoke: deps-python
	@set -e; \
	for ex in tsp maxcut; do \
		echo "==> examples/$$ex (python)"; \
		uv run --no-sync python examples/$$ex/runner.py --seed 42 --interpreter python -o /tmp/xquad-$$ex-py.json; \
		diff examples/$$ex/golden.json /tmp/xquad-$$ex-py.json; \
		echo "==> examples/$$ex (rust)"; \
		uv run --no-sync python examples/$$ex/runner.py --seed 42 --interpreter rust -o /tmp/xquad-$$ex-rust.json; \
		diff examples/$$ex/golden.json /tmp/xquad-$$ex-rust.json; \
	done

# Regenerate each example's golden.json from the current Python-path
# runner output. Use after an intentional runner / xqcp / xqsa change;
# pair with `make example-smoke` to confirm parity on both paths.
regen-example-goldens: deps-python
	uv run --no-sync python examples/tsp/runner.py --seed 42 --interpreter python -o examples/tsp/golden.json
	uv run --no-sync python examples/maxcut/runner.py --seed 42 --interpreter python -o examples/maxcut/golden.json

# -- Documentation ----------------------------------------------------------

docs: docs-check
	mdbook-mermaid install .
	mdbook build

# Regenerate docs/bytecode-semantics.md from conformance/opcodes.yaml.
docs-regen:
	uv run python scripts/gen-bytecode-docs.py

# Assert the committed docs/bytecode-semantics.md matches the regenerated
# output; used by the CI docs-build job to prevent silent drift.
docs-check:
	uv run python scripts/gen-bytecode-docs.py --check

docs-serve:
	mdbook-mermaid install .
	mdbook serve --open
