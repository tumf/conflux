## Implementation Tasks

- [ ] Trace and preserve the TUI `x` queue command path from `src/tui/key_handlers.rs`, `src/tui/state.rs`, and `src/tui/command_handlers.rs` so Running-mode bulk queue actions still emit `TuiCommand::AddToQueue` for eligible `not queued` rows. (verification: unit - `cargo test tui::state::tests::test_bulk_toggle_running_mode_emits_add_to_queue_commands tui::state::tests::test_bulk_toggle_running_mode_excludes_active_rows_from_commands` proves bulk `x` emits AddToQueue commands for `not queued` rows and does not emit stop/dequeue commands for active rows)

- [ ] Update scheduler queue ingestion/reconciliation state so newly added dynamic-queue or reducer-visible queued candidates are represented as an explicit queue-addition reanalysis trigger rather than an ordinary debounceable timer check. (verification: unit - add or update `src/parallel/tests/executor.rs` coverage for `check_dynamic_queue_and_add_changes()` / `reconcile_queued_candidates_from_shared_state()` and run `cargo test parallel::tests::executor::test_idle_queue_addition_marks_reanalysis_and_enqueues_change` plus the new test name)

- [ ] Modify `ParallelExecutor::perform_reanalysis_and_dispatch()` or its caller so `ReanalysisReason::QueueNotification` with newly available queued work starts dependency analysis immediately even when `last_queue_change_at` is fresh and `iteration > 1`. (verification: unit - add `src/parallel/tests/executor.rs` test coverage that sets `last_queue_change_at = Some(now)`, `iteration = 2`, `ReanalysisReason::QueueNotification`, and a dispatchable queued change, then run `cargo test parallel::tests::executor::<new_queue_notification_debounce_test>` and assert `AnalysisStarted` is emitted)

- [ ] Preserve zero-capacity behavior: dependency analysis must still run for explicit queue additions while ordinary apply dispatch remains suppressed when no execution slots are available. (verification: integration - add or update `src/parallel/tests/manual_resolve.rs` coverage and run `cargo test parallel::tests::manual_resolve::scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve`; evidence must include `AnalysisStarted`, `dispatch_capacity_zero_after_analysis`, and no `ApplyStarted` before capacity is released)

- [ ] Add a running scheduler regression test for the user-visible bug: after initial scheduler work has advanced beyond iteration 1, push or command-queue a new change and assert analysis starts without waiting for the 10-second debounce window. (verification: integration - add timeout-bounded test under `src/parallel/tests/manual_resolve.rs` or `src/parallel/tests/executor.rs`, run `cargo test parallel::tests::<new_running_x_queue_reanalysis_test>`, and observe `AnalysisStarted` within a sub-second timeout after dynamic queue push)

- [ ] Verify blocked-only and persistent idle behavior remains event-driven and does not reintroduce timer-driven repository/worktree polling. (verification: integration - run `cargo test parallel::tests::executor::test_persistent_idle_wait_does_not_poll_worktree_reconciliation_without_wake parallel::tests::executor::test_persistent_idle_wait_wakes_on_queue_push_and_notify_scheduler parallel::tests::executor::test_blocked_only_reanalysis_skips_analyzer_for_merge_wait_and_terminal_error`)

- [ ] Run targeted Rust tests for the affected modules and default quality gates. (verification: manual - run `cargo test parallel::tests::executor parallel::tests::manual_resolve tui::state::tests`; then run the repository's discovered default lint/typecheck/test commands, excluding heavy tests unless explicitly requested)

## Future Work

- Manual TUI smoke test: start a persistent TUI parallel run, press `x` on `not queued` rows while another change is active, and confirm the log shows `Re-analyzing queued changes for dispatch` promptly.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-queue-notification-reanalysis --archive-gate`
