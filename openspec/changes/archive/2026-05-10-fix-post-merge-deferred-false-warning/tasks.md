## Implementation Tasks

- [x] Add base-integration guard before archive-incomplete manual deferral in post-archive merge handling. (verification: unit - add/extend Rust tests around `src/parallel/merge.rs` so an already-integrated archived change returns a non-error/idempotent outcome instead of `MergeDeferred(auto_resumable=false)`)
- [x] Prevent duplicate archived-workspace re-dispatch while a post-archive merge task is active for the same change. (verification: integration - add/extend scheduler tests in `src/parallel/tests/executor.rs` covering archive-complete workspace plus active same-change merge task, asserting no second ordinary dispatch is queued)
- [x] Preserve legitimate manual deferral behavior for real dirty-base blockers. (verification: unit/integration - `src/parallel/merge.rs::tests::test_attempt_merge_dirty_base_remains_manual_deferred` verifies a dirty base still returns `MergeDeferred(auto_resumable=false)`; reducer/manual wait behavior is covered by `src/orchestration/state.rs` merge-deferred tests; runnable command: `cargo test test_attempt_merge_dirty_base_remains_manual_deferred`)
- [x] Ensure terminal success wins over stale duplicate merge outcomes in reducer-visible state. (verification: unit - add/extend `src/orchestration/state.rs` tests so `MergeCompleted` followed by stale `MergeDeferred(auto_resumable=false)` does not regress display state from `merged` to `merge wait`)
- [x] Run focused Rust regression tests for merge/reducer paths. (verification: integration - run targeted `cargo test` filters for the added `parallel` and `orchestration_state` tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-post-merge-deferred-false-warning --archive-gate`

## Acceptance #1 Failure Follow-up

Resolved in this apply pass by replacing the broad dirty-base verification note with repository-verifiable evidence: `src/parallel/merge.rs::tests::test_attempt_merge_dirty_base_remains_manual_deferred`, related reducer tests in `src/orchestration/state.rs`, and the runnable command `cargo test test_attempt_merge_dirty_base_remains_manual_deferred`.
