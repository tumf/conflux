## Implementation Tasks

- [ ] Introduce an explicit background merge task outcome type that differentiates merged, deferred, and failed results across `src/parallel/merge.rs` and `src/parallel/queue_state.rs` (verification: unit - add or update tests in `src/parallel/tests/executor.rs` or `src/parallel/tests/queue_state.rs` that construct a deferred merge path and assert it is not represented as `Ok(())` success).
- [ ] Update deferred merge handling so `MergeAttempt::Deferred` continues to emit `ParallelEvent::MergeDeferred` and workspace `MergeWait`/`ResolveWait` state updates, but reports a deferred/pending task result to queue state (verification: integration - run a filtered cargo test covering deferred merge event/status handling, expected command `cargo test merge_deferred` or the nearest repository-supported filtered test).
- [ ] Update `src/parallel/queue_state.rs` so `handle_merge_result` logs deferred merge tasks as deferred/pending and does not emit `Background merge task completed successfully` for them (verification: unit - add a test that feeds a deferred merge result into queue-state handling and asserts the success log/follow-up path is not taken).
- [ ] Preserve successful behavior for actual `MergeAttempt::Merged` including idempotent already-merged revisions (verification: unit - add or update a test that returns `MergeAttempt::Merged { revision: "already-merged-to-base" }` and asserts success handling and retry of deferred base-lane waiters still occurs).
- [ ] Preserve failed merge behavior for true errors (verification: unit - add or update a test that feeds an error merge result and asserts `ParallelEvent::Error` still includes the change id, workspace name, and error string).
- [ ] Run affected Rust verification and OpenSpec validation (verification: integration - run `cargo test` or targeted affected tests plus `cargo fmt --check`, and run `cflx openspec validate fix-deferred-merge-result-status --strict --evidence warn`).

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-deferred-merge-result-status --archive-gate`
