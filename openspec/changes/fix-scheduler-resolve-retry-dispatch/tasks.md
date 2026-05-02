## Implementation Tasks

- [x] Add a scheduler-owned dispatch point for synced `ResolveWait` intent. Completion condition: after `src/parallel/orchestration.rs` syncs reducer-owned resolve intent, the scheduler calls the existing deferred/manual retry path when `resolve_wait_changes` is non-empty, even if `queued` and `in_flight` are empty. (verification: unit - `cargo test parallel::tests::executor::test_scheduler_dispatches_synced_manual_resolve_wait_without_queued_work` or equivalent test in `src/parallel/tests/executor.rs`)

- [x] Prevent busy retry loops for unchanged blocked retry intent. Completion condition: a still-blocked auto-resumable or manual-blocked retry is not retried continuously on every 500ms scheduler tick without a new wake/retry trigger such as a manual `M`, merge completion, resolve completion, rejection completion, queue notification, or state transition. (verification: unit - `cargo test parallel::tests::executor::test_scheduler_does_not_busy_retry_unchanged_resolve_wait` or equivalent retry-trigger test)

- [x] Preserve existing completion-triggered retry behavior. Completion condition: `handle_merge_result`, rejection completion/failure, and successful resolve completion still invoke retry for deferred rows that were blocked by an active merge/resolve/reject lane. (verification: integration - existing tests around `src/parallel/queue_state.rs` and `src/parallel/merge.rs`, including rejection retry and resolve completion retry paths)

- [x] Keep TUI command ownership limited to intent and wake-up. Completion condition: `src/tui/command_handlers.rs` for `TuiCommand::ResolveMerge` still applies `ReducerCommand::ResolveMerge`, calls `notify_scheduler()`, and does not call `resolve_deferred_merge` or direct merge execution. (verification: unit - targeted command handler/state test or repository inspection paired with `cargo test tui::state::test_resolve_merge_immediate_syncs_reducer tui::state::test_resolve_merge_queues_syncs_reducer`)

- [x] Verify user-visible manual retry behavior. Completion condition: a focused test or manual run shows that after `M`, the scheduler emits retry-related behavior instead of only logging scheduled intent and leaving the row indefinitely in `resolve pending`. (verification: e2e - add/run `cargo test parallel::tests::executor::test_scheduler_dispatches_synced_manual_resolve_wait_without_queued_work` and, when checking the real scenario, run `cflx` in `/Users/tumf/wakumo/avacus/avacuscc-dbot` to confirm `/Users/tumf/.local/state/cflx/logs/**/YYYY-MM-DD.log` includes retry/merge/resolve/defer output after the scheduled-intent log)

- [x] Run required checks. Completion condition: formatting and targeted tests pass, and any test taking over 1 second is optimized or marked heavy per repository policy. (verification: integration - `cargo fmt --check`; targeted `cargo test` for affected parallel/TUI modules)

## Future Work

- If scheduler trigger semantics remain hard to reason about after this fix, propose a separate refactor to model retry triggers explicitly instead of deriving them from multiple counters and notification paths.
