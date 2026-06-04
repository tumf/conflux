## Implementation Tasks

- [x] Update dependency dispatch gating so archived dependency references still require base-branch merge evidence before dependent dispatch (verification: unit - `cargo test test_single_queued_archived_dependency_waits_until_merged` or equivalent targeted test in `src/parallel/tests/executor.rs`).
- [x] Preserve distinct diagnostics/classification for archived dependency references while preventing archived-but-not-merged from being treated as satisfied (verification: unit - add assertions in `src/parallel/tests/executor.rs` using `drain_dependency_events` and run `cargo test test_archived_dependency_is_satisfied_without_rejection`).
- [x] Add or update regression coverage for the A/B/C case where B and C depend on A and A is resolving/archived-but-not-merged (verification: unit - add a scheduler dispatch test in `src/parallel/tests/executor.rs` and run `cargo test dependency_resolving_dependents_wait_until_merged`).
- [x] Add or update coverage proving dependents become eligible after the dependency is merged to base (verification: unit/integration - add a git fixture in `src/parallel/tests/executor.rs` using `init_git_repo`/base-branch merge evidence and run the targeted cargo test by name).
- [x] Keep dependency-resolved fresh workspace recreation behavior intact when a previously blocked dependent becomes eligible (verification: unit - run `cargo test dependency_resolved` and confirm existing `force_recreate_worktree` assertions still pass or are updated in `src/parallel/tests/executor.rs`).
- [x] Run relevant quality gates for the touched Rust scheduler/test code (verification: manual - run `cargo test --test parallel` if available, otherwise targeted `cargo test` commands for `src/parallel/tests/executor.rs`, then configured lint/typecheck commands).

## Future Work

- None.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-dependency-dispatch-after-merge --archive-gate`
