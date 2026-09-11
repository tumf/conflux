## Implementation Tasks

- [x] Add `support.rs` and move the shared `TestWorkspaceManager`, builders, Git initialization, config, and failure helpers. (verification: integration - `cargo test --lib parallel::tests && cargo test --lib parallel_run_service`; verification-id: focused-tests)
- [x] Repoint sibling test modules and remove only byte-equivalent duplicate helpers. Keep non-identical local variants, including helpers in `src/parallel_run_service.rs`. (verification: integration - `cargo test --lib parallel::tests && cargo test --lib parallel_run_service`; verification-id: focused-tests)
- [x] Verify test inventory and behavior remain unchanged; do not split behavioral test groups or optimize runtime in this change. (verification: integration - `cargo test --lib parallel::tests && cargo test --lib parallel_run_service`; verification-id: focused-tests)

## Final Validation

Archive validation is authoritative. Expected archive gate: `cflx openspec validate extract-parallel-test-support --archive-gate`.

## Notes

- `src/parallel/tests/support.rs` now owns `TestAssertionExt`/`or_fail` (failure), `create_test_config` / `create_test_config_with` (config), `TestWorkspaceManager` with its builders and `WorkspaceManager` impl (workspace), and the async `init_git_repo` (Git setup). All are `pub(super)`, so siblings reach them as `super::support::*` and no sibling imports a fixture from `executor.rs` any more.
- Removed byte-equivalent duplicates of `create_test_config` from `auto_resolve.rs`, `manual_resolve.rs`, `unchanged_analysis_input.rs`, and `reanalysis_trigger_lifetime.rs`; all four bodies were character-identical to the extracted one.
- Kept the non-identical local `init_git_repo` variants in `auto_resolve.rs` (different base commit content/message, `expect` error handling) and `workspace_resume.rs` (synchronous, identity-only, no base commit), each with a comment stating why it is not the shared helper. `src/parallel_run_service.rs` is untouched.
- The two `or_fail` behavior tests stay in `executor.rs` so no test changes module path; the trait itself is imported from `support.rs`.
- evidence: test inventory diff of `cargo test --lib parallel::tests -- --list` before and after is IDENTICAL (358 entries, same fully-qualified names).
- evidence: `cargo test --lib parallel::tests` -> ok, 355 passed, 0 failed, 3 ignored.
- evidence: `cargo test --lib parallel_run_service` -> ok, 20 passed, 0 failed.
- evidence: `cargo fmt`, `cargo check --lib --profile test` (default and `--features heavy-tests`), and `cargo clippy --lib --all-targets` all clean with no warnings.

## Future Work

- None.
