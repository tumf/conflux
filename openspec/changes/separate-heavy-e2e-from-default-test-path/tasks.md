## Implementation Tasks

- [ ] Inventory the current `tests/` surface and classify each suite as fast default-path, fast integration, contract, or heavy E2E/real-boundary validation (verification: the classification references concrete files such as `tests/e2e_proposal_session.rs`, `tests/e2e_git_worktree_tests.rs`, `tests/process_cleanup_test.rs`, and `tests/merge_conflict_check_tests.rs`).
- [ ] Define the repository mechanism for separating heavy tests from the default test path (for example file layout, ignored tests, dedicated commands, feature gates, or equivalent) and document the chosen rule in a way that matches Cargo/Rust conventions used by this repo (verification: repository docs/config/tests clearly show how to run default-path vs heavy-path tests).
- [ ] Reorganize or mark heavy test suites so default developer validation does not implicitly include real-boundary E2E coverage (verification: heavy suites are moved, tagged, or otherwise excluded from the default path by design).
- [ ] Update repository-standard validation guidance so pre-commit, local developer loops, acceptance, and broader validation each call the appropriate test tier (verification: commands or docs under repository-controlled files reflect the new separation).
- [ ] Preserve explicit execution paths for heavy test suites, including clear commands for running them intentionally (verification: a documented command or command set runs the heavy tier on demand).
- [ ] Add regression checks or documentation tests as needed to ensure newly added heavy tests do not silently drift back into the default path (verification: repository tests/docs enforce or describe the boundary).
- [ ] Run repository verification for the new structure (`cargo fmt --check`, `cargo clippy -- -D warnings`, and the intended default-path test command) and separately verify the heavy tier command still works (verification: all relevant commands succeed).

## Future Work

- Add CI lane separation if the project later wants different schedules or requiredness for heavy E2E tiers.
- Introduce richer per-test metadata if the team wants machine-enforced categorization beyond naming/layout conventions.
