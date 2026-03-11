.PHONY: all deps lint lint-clippy lint-fmt lint-taplo lint-doc lint-semver lint-deny test test-unit test-integration test-miri

all: lint test

# -- Dependencies -----------------------------------------------------------

deps:
	rustup component add clippy rustfmt
	rustup toolchain install nightly --component miri
	cargo +nightly miri setup
	cargo install taplo-cli cargo-semver-checks cargo-deny cargo-nextest --locked

# -- Lints ------------------------------------------------------------------

lint: lint-clippy lint-fmt lint-taplo lint-doc lint-deny

lint-clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

lint-fmt:
	cargo fmt --all -- --check

lint-taplo:
	taplo lint check
	taplo fmt --check

lint-doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

lint-semver:
	cargo semver-checks check-release --workspace

lint-deny:
	cargo deny check

# -- Tests ------------------------------------------------------------------

test: test-unit test-integration

test-unit:
	cargo nextest run --workspace --all-features --lib --profile ci-test

test-integration:
	cargo nextest run --workspace --all-features --test '*' --profile ci-test

test-miri:
	cargo +nightly miri test --workspace --all-features
