## Implementation Tasks

- [x] Normalize manual `M` retry ownership so `AppState::resolve_merge()` and `TuiCommand::ResolveMerge` cannot leave display state ahead of reducer-owned scheduler intent. Completion condition: the selected row only remains `resolve pending` when the shared reducer contains scheduler-consumable retry membership, or it returns to `merge wait` with blocker evidence. (verification: unit - add or update TUI state/command-handler tests covering reducer acceptance and NoOp handling in `src/tui/state.rs` and `src/tui/command_handlers.rs`.)

- [x] Ensure clean retry after dirty manual deferral is accepted as fresh scheduler work. Completion condition: after `MergeDeferred(auto_resumable=false)` or dirty-base `ResolveFailed` returns a change to `MergeWait`, a later explicit `ResolveMerge` for the same change updates reducer state to `ResolveWait` and invalidates stale retry suppression for that change. (verification: unit - add reducer tests in `src/orchestration/state.rs` for dirty manual deferral followed by explicit retry after clean preconditions.)

- [x] Ensure scheduler dispatch consumes accepted retry intent when no normal queued work exists. Completion condition: if no scheduler is running, `TuiCommand::ResolveMerge` starts a manual retry scheduler run that evaluates reducer-owned resolve waiters instead of reporting successful zero-change completion; if a scheduler is running, it is notified and evaluates base-lane waiters. (verification: integration - add or update parallel executor/command-handler tests in `src/parallel/tests/executor.rs` and/or `src/tui/command_handlers.rs` proving retry dispatch reaches the merge attempt path without another queued change or keypress.)

- [x] Prevent executor-local stale state from suppressing explicit clean retry. Completion condition: previous `last_dispatched_resolve_wait_changes`, dirty tracking, or local wait caches are reset or bypassed when a user-issued retry intent is accepted after manual `merge wait`. (verification: integration - add a regression test in `src/parallel/tests/executor.rs` where the first retry is blocked by dirty state, the blocker is cleared, and the second retry is dispatched for the same change.)

- [x] Preserve terminal-state safety for stale manual retry input. Completion condition: stale `M` or `ResolveMerge` for `merged`, `rejected`, or other permanent final states remains NoOp and does not create `ResolveWait` membership or scheduler work. (verification: unit - add reducer/TUI tests in `src/orchestration/state.rs` and/or `src/tui/state.rs` asserting terminal states are not reintroduced as retry work.)

- [x] Run targeted Rust verification for the changed modules. Completion condition: targeted tests for TUI resolve, command handling, orchestration reducer, and parallel retry dispatch pass locally. (verification: manual - run targeted `cargo test` commands for `src/tui/state.rs`, `src/tui/command_handlers.rs`, `src/orchestration/state.rs`, and `src/parallel/tests/executor.rs`; this is intentional manual coverage because exact command filters depend on final edited tests.)

## Future Work

Broader scheduler starvation work remains separate and should continue under existing scheduler-related proposals/issues if needed.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected authoring checks:

`cflx openspec validate fix-tui-merge-wait-clean-retry --strict --evidence warn`

`cflx openspec validate fix-tui-merge-wait-clean-retry --archive-gate`
