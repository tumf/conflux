## Implementation Tasks

- [ ] Update `rustfmt` and `clippy` in `.pre-commit-config.yaml` to remove `always_run: true` and apply `files: ^(src|tests)/.*\.rs$|^Cargo\.(toml|lock)$|^build\.rs$`, preserving their existing commands and `pass_filenames: false`. (verification: integration - runs against stable existing non-Rust files (`README.md` and `openspec/AGENTS.md`) must report `Skipped`, while matching runs with `src/main.rs` must report `Passed`; verification-id: precommit-selection-tests)
- [ ] Add repository-local regression coverage or a deterministic validation command that asserts both Rust hooks share the exact selector, retain full-workspace commands and `pass_filenames: false`, and contain no `always_run` field. (verification: integration - a tracked test or script run by `make test` reads `.pre-commit-config.yaml` and fails when either selector, command, `pass_filenames`, or `always_run` behavior drifts; verification-id: precommit-selection-tests)
- [ ] Update `CONTRIBUTING.md` and `docs/guides/DEVELOPMENT.md` to distinguish path-scoped commit-time Rust checks from explicit full validation, and make explicit manual Rust-hook examples use `--all-files`. (verification: integration - a tracked documentation assertion or `rg 'proposal-only|path-scoped|make check|--all-files' CONTRIBUTING.md docs/guides/DEVELOPMENT.md` confirms both behaviors are stated; verification-id: precommit-selection-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate scope-rust-precommit-hooks --archive-gate`.
