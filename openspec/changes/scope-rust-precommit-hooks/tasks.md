## Implementation Tasks

- [x] Update `rustfmt` and `clippy` in `.pre-commit-config.yaml` to remove `always_run: true` and apply `files: ^(src|tests)/.*\.rs$|^Cargo\.(toml|lock)$|^build\.rs$`, preserving their existing commands and `pass_filenames: false`. (verification: integration - runs against stable existing non-Rust files (`README.md` and `openspec/CONSTITUTION.md`) must report `Skipped`, while matching runs with `src/main.rs` must report `Passed`; verification-id: precommit-selection-tests)
- [x] Add repository-local regression coverage or a deterministic validation command that asserts both Rust hooks share the exact selector, retain full-workspace commands and `pass_filenames: false`, and contain no `always_run` field. (verification: integration - `tests/precommit_hook_scope_tests.rs` runs under `make test` and reads `.pre-commit-config.yaml`, failing when either selector, command, `pass_filenames`, or `always_run` behavior drifts; verification-id: precommit-selection-tests)
- [x] Update `CONTRIBUTING.md` and `docs/guides/DEVELOPMENT.md` to distinguish path-scoped commit-time Rust checks from explicit full validation, and make explicit manual Rust-hook examples use `--all-files`. (verification: integration - `development_docs_state_the_selection_contract` in `tests/precommit_hook_scope_tests.rs` asserts both documents state `path-scoped`, `proposal-only`, `make check`, and `--all-files`; verification-id: precommit-selection-tests)

## Notes

- The original task text and the proposal `rerun` command named `openspec/AGENTS.md` as a stable non-Rust file, but that file was deleted from the repository in commit `8db0f443`. A nonexistent path is skipped by every hook regardless of selection, so it proved nothing; both were changed to the tracked `openspec/CONSTITUTION.md`.
- evidence: `prek run rustfmt --files README.md`, `prek run clippy --files openspec/CONSTITUTION.md`, and `prek run rustfmt --files docs/guides/DEVELOPMENT.md` all report `Skipped`; `src/main.rs`, `tests/release_bump_scope_tests.rs`, `Cargo.toml`, `Cargo.lock`, and `build.rs` all report `Passed` for both hooks.
- evidence: `cargo test --test precommit_hook_scope_tests` — 6 passed, 0 failed, 0.00s.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate scope-rust-precommit-hooks --archive-gate`.
