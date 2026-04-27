## Summary

<!-- Describe what this MR changes and why. Reference related issues with #issue-number. -->

## Type of Change
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update

## License Compliance Checklist
- [ ] All new files include the required AGPL-3.0-or-later license header
- [ ] Any new dependencies are AGPL-3.0-or-later compatible
- [ ] NOTICE file updated if adding AGPL-3.0-or-later compatible dependencies.
- [ ] `deny.toml` updated if the license is not listed and its compatible with AGPL-3.0-or-later
- [ ] No proprietary or incompatible code was incorporated

## Checklist

- [ ] Commit subject lines are 72 characters or fewer and use the imperative mood
- [ ] Commits are signed off (`git commit -s`)
- [ ] `cargo fmt --all` run
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes
- [ ] `cargo doc --workspace --all-features --no-deps` builds without warnings
- [ ] `cargo deny check` passes
- [ ] `cargo test --workspace --all-features` passes
- [ ] `cargo +nightly miri test --workspace --all-features` passes (or N/A)
- [ ] All public items are documented
- [ ] New source files include the AGPL license header
