## Implementation Tasks

- [ ] Reproduce and characterize the stuck manual resolve path. Completion condition: a failing regression test or focused harness demonstrates that a `MergeWait` row becomes reducer-owned `ResolveWait` after `M`, while scheduler-owned `resolve_wait_changes` or equivalent runnable-work detection remains empty before the fix. (verification: unit - `cargo test parallel::tests::executor::test_scheduler_syncs_manual_resolve_wait_from_shared_state` or equivalent targeted test)

- [ ] Sync reducer-owned `ResolveWait` intent into the scheduler before idle/drained decisions. Completion condition: `src/parallel/orchestration.rs` or adjacent scheduler code reads shared `OrchestratorState::resolve_wait_change_ids()` or an equivalent reducer-owned source before deciding all work is complete or waiting as idle, so manual resolve intent prevents false drain. (verification: unit - `cargo test parallel::tests::executor::test_scheduler_syncs_manual_resolve_wait_from_shared_state` or an equivalent scheduler state test covering `should_exit_when_idle`/drained decision with shared resolve intent)

- [ ] Ensure manual resolve retry is executed through the scheduler-owned merge/retry path. Completion condition: `TuiCommand::ResolveMerge` continues to record shared reducer intent and wake the scheduler, and no direct TUI-owned merge execution path is reintroduced. (verification: integration - targeted TUI command handler/state tests for `ResolveMerge` plus code inspection of `src/tui/command_handlers.rs`)

- [ ] Preserve refresh and queued resolve behavior. Completion condition: `ChangesRefreshed` or workspace archived observations do not regress a manual `ResolveWait` row back to `MergeWait` before the scheduler consumes it, and queued resolve behavior while another resolve is active still works. (verification: unit - `cargo test tui::state::test_resolve_merge_queues_syncs_reducer tui::state::test_resolve_merge_immediate_syncs_reducer` plus a regression test in `src/tui/state.rs` or `src/orchestration/state.rs` for refresh after `M`)

- [ ] Verify merge/retry completion semantics. Completion condition: successful retry transitions to `merged`, non-auto-resumable deferral returns to `merge wait`, auto-resumable deferral remains `resolve pending`, and failure clears or restores reducer intent according to existing events. (verification: integration - `cargo test orchestration::state::tests::test_resolve_completed_clears_stale_resolve_wait_after_refresh` plus targeted `src/parallel/tests/executor.rs` tests for `MergeCompleted`, `MergeDeferred`, and `ResolveFailed` paths)

- [ ] Run required Rust checks. Completion condition: formatting and targeted tests pass, and any test taking over 1 second is either optimized or marked heavy per repository policy. (verification: integration - `cargo fmt --check`; targeted `cargo test` for affected parallel/TUI modules)

## Future Work

- Broader cleanup of duplicated or repeated canonical requirements in `openspec/specs/orchestration-state/spec.md` and `openspec/specs/parallel-merge/spec.md` should be handled in a separate spec hygiene proposal.
