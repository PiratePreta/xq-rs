# Contributing to Aglais XQVM

Thank you for your interest in contributing. This document covers the development workflow and requirements for getting changes merged.

## Prerequisites

- Rust (stable, latest recommended)
- `cargo clippy` — `rustup component add clippy`
- `cargo fmt` — `rustup component add rustfmt`
- `cargo deny` — `cargo install cargo-deny --locked`
- `cargo semver-checks` — `cargo install cargo-semver-checks --locked`

## Development Workflow

All checks must pass before a merge request is accepted. Run them locally before pushing:

```sh
# Formatting
cargo fmt --all

# Lints (must be warning-free)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Documentation (must be warning-free)
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

# License compliance
cargo deny check licenses

# Tests
cargo test --workspace --all-features
```

## Semver Compliance

Public API changes must be semver-compatible. `cargo semver-checks` runs automatically on the default branch and on version tags. Breaking changes require a major version bump.

## Code Style

- All public items must be documented (`missing-docs` is enforced).
- `unsafe` code is not allowed (`unsafe-code = "deny"`).
- Follow standard Rust naming conventions (`nonstandard-style = "deny"`).
- Run `cargo fmt` before committing — formatting is checked in CI.

## Licensing

By submitting a contribution, you agree that your work will be licensed under [AGPL-3.0-or-later](https://www.gnu.org/licenses/agpl-3.0.html), the same license as this project.

## Conduct

Be respectful in all project spaces, including issues, merge requests, and code review.

## Merge Requests

- Keep changes focused and minimal.
- Reference any related issues in the MR description.
- Ensure all CI pipeline stages pass.
