## Implementation Tasks

- [ ] Update `rustfmt` and `clippy` in `.pre-commit-config.yaml` to remove `always_run: true` and apply `files: ^(src|tests)/.*\.rs$|^Cargo\.(toml|lock)$|^build\.rs$`, preserving their existing commands and `pass_filenames: false`. (verification: integration - `prek run rustfmt --files openspec/changes/example/proposal.md`, `prek run clippy --files openspec/changes/example/proposal.md`, and matching runs with `src/main.rs` prove skip and selection behavior; verification-id: precommit-selection-tests)
- [ ] Add repository-local regression coverage or a deterministic validation command that asserts both Rust hooks share the exact selector and retain full-workspace commands. (verification: integration - a tracked test or script run by `make test` reads `.pre-commit-config.yaml` and fails when either selector, command, or `pass_filenames` behavior drifts; verification-id: precommit-selection-tests)
- [ ] Update `CONTRIBUTING.md` and `docs/guides/DEVELOPMENT.md` to distinguish path-scoped commit-time Rust checks from explicit full validation. (verification: integration - a tracked documentation assertion or `rg 'proposal-only|path-scoped|make check' CONTRIBUTING.md docs/guides/DEVELOPMENT.md` confirms both behaviors are stated; verification-id: precommit-selection-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate scope-rust-precommit-hooks --archive-gate`.
