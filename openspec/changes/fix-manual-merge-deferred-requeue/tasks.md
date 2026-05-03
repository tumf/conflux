# Tasks

## Implementation Tasks

- [ ] Update reducer handling for manual merge deferral in `src/orchestration/state.rs` so `ExecutionEvent::MergeDeferred { auto_resumable: false, .. }` sets `WaitState::MergeWait`, clears `QueueIntent::Queued` to `QueueIntent::NotQueued`, removes the change from reducer-owned `resolve_wait_queue`, and leaves the display status as `merge wait`. (verification: unit - add or update `#[cfg(test)]` coverage in `src/orchestration/state.rs` and run `cargo test manual_merge_deferred --lib`; completion condition: a reducer runtime entry that was previously queued no longer appears in `queued_change_ids()` after manual merge deferral.)

- [ ] Preserve auto-resumable merge deferral semantics in `src/orchestration/state.rs` for `MergeDeferred { auto_resumable: true, .. }`, including `resolve pending` display, `resolve_wait_change_ids()` membership, and survival across workspace refresh. (verification: unit - keep or extend `test_auto_resumable_merge_deferred_sets_resolve_wait` and related tests in `src/orchestration/state.rs`, then run `cargo test auto_resumable_merge_deferred --lib`; completion condition: auto-resumable deferrals remain scheduler retry intent and do not regress to `merge wait`.)

- [ ] Add scheduler reconciliation regression coverage for manual merge-deferred changes in `src/parallel/queue_state.rs` tests or `src/parallel/tests/executor.rs`. (verification: unit - add a Rust test in `src/parallel/tests/executor.rs` that initializes reducer queued intent, applies manual `MergeDeferred(false)`, runs `reconcile_queued_candidates_from_shared_state()`, and assert the scheduler-local queued list does not add that change; completion condition: manual merge-deferred changes are not reintroduced as ordinary queued candidates.)

- [ ] Verify explicit retry via `ReducerCommand::ResolveMerge` still works after manual deferral without normal queue intent. (verification: unit - add or update reducer/TUI state tests in `src/orchestration/state.rs` or `src/tui/state.rs` showing `ResolveMerge` on a manual merge-deferred change sets `ResolveWait` and returns the change from `resolve_wait_change_ids()`; completion condition: a manually merge-deferred change can transition from `merge wait` to `resolve pending` when explicit retry intent is applied.)

- [ ] Run targeted Rust tests for reducer and scheduler behavior and record results in the implementation summary. (verification: manual - run targeted `cargo test` filters for `orchestration::state` merge-deferred tests and parallel queue reconciliation tests; completion condition: targeted tests complete successfully and any test over 1 second is optimized or marked heavy according to repository policy.)

## Future Work

- End-to-end reproduction against `/Users/tumf/wakumo/avacus/avacuscc-dbot` is optional manual validation after implementation because it depends on a dirty base workspace and a preserved archived worktree state.
