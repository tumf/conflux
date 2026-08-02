## Implementation Tasks

- [ ] Define the minimal typed ownership boundary for background merge outcomes so exhausted post-archive resolve failures remain change-scoped and genuinely run-fatal failures remain distinguishable without diagnostic substring matching. Completion requires exhaustive matches and tests for both scopes. (verification: unit - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [ ] Rewire conflict exhaustion and post-archive merge handling to emit exactly one authoritative `ResolveFailed { change_id, error }` transition and remove merge-layer generic Error emission for that already-classified failure. Completion requires a collected event sequence containing the structured change-local failure and no generic global Error. (verification: integration - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [ ] Update background merge result handling to release lane/counter ownership and preserve scheduler progress without wrapping change-local failures in another `ParallelEvent::Error`; retain global Error emission for typed run-fatal outcomes. Completion requires tests for PostArchiveMerge, retry origins, lane release, duplicate suppression, and a genuine fatal control case. (verification: integration - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [ ] Add cross-layer reducer and TUI regressions that apply the actual post-archive failure event sequence and prove the failed change remains `merge wait`, worktree evidence remains retryable, execution mode never becomes Error, and existing active-work rules choose Running or Select as appropriate. Completion requires a separate global fatal event to still enter TUI Error. (verification: integration - `cargo test --lib tui::` and `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [ ] Add scheduler continuation coverage proving an unrelated eligible change can still dispatch after one post-archive resolve exhaustion and that no success/completion event is synthesized for the failed change. Completion requires deterministic channel/service-double evidence in the default test suite. (verification: integration - `cargo test --lib parallel::tests::`; verification-id: change-local-merge-error-tests)

- [ ] Run the complete repository-local gate and resolve formatting, lint, compile, and test failures introduced by the event reclassification without broadening unrelated error semantics. Completion requires every command to exit successfully. (verification: integration - `cargo test --lib parallel::tests:: && cargo test --lib tui:: && cargo fmt --check && cargo clippy -- -D warnings`; verification-id: change-local-merge-error-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-change-local-merge-error-scope --archive-gate`.

## Future Work

- Review other generic `ParallelEvent::Error` producers in separate changes only when concrete evidence shows a change-local or recoverable outcome is misclassified.
