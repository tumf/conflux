## Implementation Tasks

- [ ] Task 1: Add post-archive merge/resolve dispatch in TUI orchestrator (verification: `src/tui/orchestrator.rs` emits `MergeDeferred` or `ResolveMerge` command after `ChangeArchived` in parallel mode; after `shared_state.apply_execution_event(&change_archived_event)` at L800-803, check `is_resolving` state; if resolve is active send `MergeDeferred { auto_resumable: true }`, otherwise send `TuiCommand::ResolveMerge`)
- [ ] Task 2: Apply `MergeDeferred` event to shared reducer in TUI orchestrator (verification: shared reducer reflects `ResolveWait` for the archived change when resolve is active; the emitted `MergeDeferred` event must also be applied to `shared_state` via `apply_execution_event`)
- [ ] Task 3: Add unit test for archive-during-resolve scenario (verification: `cargo test test_tui_archived_during_resolve` passes; simulate `ChangeArchived` then `MergeDeferred(auto_resumable=true)` sequence; assert `QueueStatus::ResolveWait` and change added to resolve queue)
- [ ] Task 4: Add unit test for archive-no-resolve scenario (verification: `cargo test test_tui_archived_no_active_resolve` passes; when no resolve is active, archived change gets immediate merge attempt)
- [ ] Task 5: Verify all existing tests pass (verification: `cargo test` succeeds with no failures)
- [ ] Task 6: Verify lint and clippy pass (verification: `cargo fmt --check && cargo clippy -- -D warnings`)

## Future Work

- Consider unifying TUI orchestrator's post-archive merge logic with headless parallel executor's `attempt_merge` to reduce divergence
