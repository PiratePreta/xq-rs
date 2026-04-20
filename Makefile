.PHONY: all \
        deps deps-docs deps-miri deps-python \
        lint lint-clippy lint-doc lint-deny \
        fmt fmt-rust fmt-taplo fmt-check fmt-check-rust fmt-check-taplo \
        test test-unit test-integration test-doc test-miri test-python \
        opcode-parity opcode-parity-rust opcode-parity-python \
        conformance conformance-rust conformance-python \
        docs docs-regen docs-check docs-serve

all: fmt lint test

# -- Dependencies -----------------------------------------------------------

deps: deps-docs
	rustup component add clippy rustfmt
	cargo install taplo-cli cargo-deny cargo-nextest --locked

deps-docs:
	cargo install mdbook mdbook-mermaid --locked

deps-miri:
	rustup toolchain install nightly --component miri
	cargo +nightly miri setup

deps-python:
	python3 -m pip install --user --upgrade pyyaml pytest ruff

# -- Formatting -------------------------------------------------------------

fmt: fmt-rust fmt-taplo

fmt-rust:
	cargo fmt --all

fmt-taplo:
	taplo fmt

fmt-check: fmt-check-rust fmt-check-taplo

fmt-check-rust:
	cargo fmt --all -- --check

fmt-check-taplo:
	taplo fmt --check

# -- Lints ------------------------------------------------------------------

lint: lint-clippy lint-doc lint-deny fmt-check

lint-clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

lint-doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

lint-deny:
	cargo deny check

# -- Tests ------------------------------------------------------------------

test: test-unit test-integration test-doc

test-unit:
	cargo nextest run --workspace --all-features --lib --cargo-profile ci-test

test-integration:
	cargo nextest run --workspace --all-features --test '*' --cargo-profile ci-test

# nextest cannot execute rustdoc doctests, so they are driven by the
# built-in test harness on a dedicated target.
test-doc:
	cargo test --doc --workspace --all-features --profile ci-test

test-miri:
	cargo +nightly miri test --workspace --all-features

test-python:
	cd xqvm-py && python3 -m pytest tests/

# -- Conformance ------------------------------------------------------------

# Cross-implementation parity (opcode table, spec conformance vectors).
# `cargo build -p xqvm` exercises the compile-time YAML ↔ opcodes! macro
# check via xqvm/build.rs; the Python script covers the xqvm-py side.
opcode-parity: opcode-parity-rust opcode-parity-python

opcode-parity-rust:
	cargo build -p xqvm

opcode-parity-python:
	python3 scripts/check-opcode-parity.py

conformance: conformance-rust conformance-python

conformance-rust:
	cargo test -p xquad-conformance --no-default-features --features rust

conformance-python:
	cargo test -p xquad-conformance --no-default-features --features python

# -- Documentation ----------------------------------------------------------

docs: docs-check
	mdbook-mermaid install .
	mdbook build

# Regenerate docs/bytecode-semantics.md from conformance/opcodes.yaml.
docs-regen:
	python3 scripts/gen-bytecode-docs.py

# Assert the committed docs/bytecode-semantics.md matches the regenerated
# output; used by the CI docs-build job to prevent silent drift.
docs-check:
	python3 scripts/gen-bytecode-docs.py --check

docs-serve:
	mdbook-mermaid install .
	mdbook serve --open
