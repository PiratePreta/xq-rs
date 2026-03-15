<!--
Copyright (C) 2026 Postquant Labs Incorporated
SPDX-License-Identifier: AGPL-3.0-or-later
-->
# Contributing to Aglais XQVM

Thank you for your interest in contributing. This document covers the development workflow and requirements for getting changes merged.

## Conduct

Be respectful in all project spaces, including issues, merge requests, and code review.

## Prerequisites

- Rust (stable, latest recommended)
- `cargo clippy` — `rustup component add clippy`
- `cargo fmt` — `rustup component add rustfmt`
- `taplo` — `cargo install taplo-cli --locked`
- `cargo deny` — `cargo install cargo-deny --locked`
- `cargo semver-checks` — `cargo install cargo-semver-checks --locked`
- `cargo nextest` — `cargo install cargo-nextest --locked` (optional, but highly recommended)
- Miri interpreter — optional, but highly recommended; see [Undefined Behaviour](#undefined-behaviour)

## Development Workflow

All checks must pass before a merge request is accepted. Run them locally before pushing:

```sh
# Formatting
cargo fmt --all
taplo fmt

# Lints (must be warning-free)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Documentation (must be warning-free)
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

# Deny checks
cargo deny check

# Tests
cargo test --workspace --all-features # OR
cargo nextest run --workspace --all-features
```

## Commit Messages

- Subject line: 50 characters or fewer, written in the imperative mood ("Add feature", not "Added feature")
- Separate subject from body with a blank line
- Body: wrap at 72 characters; explain *what* and *why*, not *how*
- Reference issues at the end of the body where applicable:
  - `Fixes #number` — closes a bug report
  - `Implements #number` — closes a feature request
  - `Reverts #number` — references a revert

Example:

```
Add opcode encoding for single-qubit gates

Introduce the initial set of single-qubit gate opcodes to the bytecode
definition. Each opcode maps to a standard gate identifier and carries
a target qubit operand.

Implements #12
```

## Semver Compliance

Public API changes must be semver-compatible. `cargo semver-checks` runs automatically on the default branch and on version tags. Breaking changes require a major version bump.

## Undefined Behaviour

Miri is not required to pass before merging, but running it locally is highly
recommended before submitting changes that touch `unsafe` code, dependencies,
or procedural macros. It detects undefined behaviour and unsound code that the
compiler and standard tests cannot catch.

Run Miri locally on nightly:

```sh
rustup toolchain install nightly --component miri
cargo +nightly miri setup
cargo +nightly miri test --workspace --all-features
```

## Code Style

- All public items must be documented (`missing-docs` is enforced).
- Follow standard Rust naming conventions (`nonstandard-style = "deny"`).
- Run `cargo fmt` and `taplo fmt` before committing — formatting is checked in CI.

## Licensing

By submitting a contribution, you agree that your work will be licensed under [AGPL-3.0-or-later](https://www.gnu.org/licenses/agpl-3.0.html), the same license as this project.

Every new source file must include the AGPL license header at the top:

```rust
// Copyright (C) <year> Postquant Labs Incorporated
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: AGPL-3.0-or-later
```

## Contributor License Agreement

By submitting a contribution to this project, you:

1. Certify that you wrote the contribution or have the right to submit it under the AGPL-3.0 license
2. Agree that your contribution will be licensed under AGPL-3.0-or-later
3. Grant a patent license as specified in the AGPL-3.0 license
4. Acknowledge that your contribution is public and may be redistributed under the AGPL-3.0 license

## Sign-off Procedure

Add a Signed-off-by line to your commit messages:

```sh
git commit -s -m "Your commit message"
```

This adds:
```
Signed-off-by: Your Name <your.email@example.com>
```

## Merge Requests

- Keep changes focused and minimal.
- Reference any related issues in the MR description.
- Ensure all CI pipeline stages pass. Use the checklist in the template.

---

**License**: This document is licensed under AGPL-3.0-or-later
**Copyright**: (C) 2026 Postquant Labs Incorporated
