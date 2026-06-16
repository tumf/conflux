## Implementation Tasks

- [x] Trace and preserve the TUI `x` queue command path from `src/tui/key_handlers.rs`, `src/tui/state.rs`, and `src/tui/command_handlers.rs` so Running-mode bulk queue actions still emit `TuiCommand::AddToQueue` for eligible `not queued` rows. (verification: unit - `cargo test tui::state::tests::test_bulk_toggle_running_mode_emits_add_to_queue_commands` and `cargo test tui::state::tests::test_bulk_toggle_running_mode_excludes_active_rows_from_commands` passed via agent-exec jobs 73732ffbb7d0f8963289d6c0b42a7250 and 1d7c3bdec1158181f77188973f95dd01, proving bulk `x` emits AddToQueue commands for `not queued` rows and does not emit stop/dequeue commands for active rows)

- [x] Update scheduler queue ingestion/reconciliation state so newly added dynamic-queue or reducer-visible queued candidates are represented as an explicit queue-addition reanalysis trigger rather than an ordinary debounceable timer check. (verification: unit - `cargo test parallel::tests::executor::test_idle_queue_addition_marks_reanalysis_and_enqueues_change` passed via agent-exec job ee1ca210e56b73159548227c649cadbd; added and ran `cargo test parallel::tests::executor::test_reducer_visible_queue_addition_marks_reanalysis_timestamp_and_enqueues_change` via agent-exec job 412b5c7098b60573be79a224d3eec931)

- [x] Modify `ParallelExecutor::perform_reanalysis_and_dispatch()` or its caller so `ReanalysisReason::QueueNotification` with newly available queued work starts dependency analysis immediately even when `last_queue_change_at` is fresh and `iteration > 1`. (verification: unit - added and ran `cargo test parallel::tests::executor::test_queue_notification_with_fresh_debounce_starts_analysis_after_initial_iteration` via agent-exec job f7c872fcc98406caf342500d868e2770; test sets `last_queue_change_at = Some(now)`, `iteration = 2`, `ReanalysisReason::QueueNotification`, a dispatchable queued change, and asserts `AnalysisStarted` with `trigger=queue` is emitted)

- [x] Preserve zero-capacity behavior: dependency analysis must still run for explicit queue additions while ordinary apply dispatch remains suppressed when no execution slots are available. (verification: integration - `cargo test parallel::tests::manual_resolve::scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve` passed via agent-exec job cf9a5e923bd6cd72b52f4b6a8cec83e9; coverage asserts `AnalysisStarted`, `dispatch_capacity_zero_after_analysis`, and no `ApplyStarted` while capacity is gated)

- [x] Add a running scheduler regression test for the user-visible bug: after initial scheduler work has advanced beyond iteration 1, push or command-queue a new change and assert analysis starts without waiting for the 10-second debounce window. (verification: integration - added and ran `cargo test parallel::tests::manual_resolve::persistent_scheduler_dynamic_queue_push_after_initial_analysis_bypasses_debounce` via agent-exec job 5d14993c037b0545facf5f50023e6b04; test observes `AnalysisStarted` for the dynamic queued change within a 500ms timeout after `DynamicQueue::push`)

- [x] Verify blocked-only and persistent idle behavior remains event-driven and does not reintroduce timer-driven repository/worktree polling. (verification: integration - ran `cargo test parallel::tests::executor::test_persistent_idle_wait_does_not_poll_worktree_reconciliation_without_wake` via agent-exec job b688c6b027bf10f9090dc099d799fe33, `cargo test parallel::tests::executor::test_persistent_idle_wait_wakes_on_queue_push_and_notify_scheduler` via job 4dad6d1dc758646bf46f48c18088c787, and `cargo test parallel::tests::executor::test_blocked_only_reanalysis_skips_analyzer_for_merge_wait_and_terminal_error` via job a73e251340f7da4edc8f38a120cfe058)

- [x] Run targeted Rust tests for the affected modules and default quality gates. (verification: manual - ran `cargo test parallel::tests::executor` via agent-exec job 2e4b15ef5ce5cc17a50bbb9b3ea9c55c, `cargo test parallel::tests::manual_resolve` via job 052421531221fffe912db14944a19630, `cargo test tui::state::tests` via job 6ea38925e387fcd2cfe01edd3880e6b5, and repository default `make check` via job 7ab77c47e89e33091e3357684a9aff97; all passed, heavy tests were not run by default per AGENTS.md)

## Future Work

- Manual TUI smoke test: start a persistent TUI parallel run, press `x` on `not queued` rows while another change is active, and confirm the log shows `Re-analyzing queued changes for dispatch` promptly.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-queue-notification-reanalysis --archive-gate`
