## Implementation Tasks

- [ ] Add base-integration guard before archive-incomplete manual deferral in post-archive merge handling. (verification: unit - add/extend Rust tests around `src/parallel/merge.rs` so an already-integrated archived change returns a non-error/idempotent outcome instead of `MergeDeferred(auto_resumable=false)`)
- [ ] Prevent duplicate archived-workspace re-dispatch while a post-archive merge task is active for the same change. (verification: integration - add/extend scheduler tests in `src/parallel/tests/executor.rs` covering archive-complete workspace plus active same-change merge task, asserting no second ordinary dispatch is queued)
- [ ] Preserve legitimate manual deferral behavior for real dirty-base blockers. (verification: unit - keep or add a test where base has uncommitted changes before merge and the result remains `MergeDeferred(auto_resumable=false)` / `MergeWait`)
- [ ] Ensure terminal success wins over stale duplicate merge outcomes in reducer-visible state. (verification: unit - add/extend `src/orchestration/state.rs` tests so `MergeCompleted` followed by stale `MergeDeferred(auto_resumable=false)` does not regress display state from `merged` to `merge wait`)
- [ ] Run focused Rust regression tests for merge/reducer paths. (verification: integration - run targeted `cargo test` filters for the added `parallel` and `orchestration_state` tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-post-merge-deferred-false-warning --archive-gate`
