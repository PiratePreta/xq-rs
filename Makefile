.PHONY: all \
        deps deps-docs deps-miri \
        lint lint-clippy lint-doc lint-deny \
        fmt fmt-rust fmt-taplo fmt-check fmt-check-rust fmt-check-taplo \
        test test-unit test-integration test-miri \
        docs docs-serve

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

test: test-unit test-integration

test-unit:
	cargo nextest run --workspace --all-features --lib --cargo-profile ci-test

test-integration:
	cargo nextest run --workspace --all-features --test '*' --cargo-profile ci-test

test-miri:
	cargo +nightly miri test --workspace --all-features

# -- Documentation ----------------------------------------------------------

docs:
	mdbook-mermaid install .
	mdbook build

docs-serve:
	mdbook-mermaid install .
	mdbook serve --open
