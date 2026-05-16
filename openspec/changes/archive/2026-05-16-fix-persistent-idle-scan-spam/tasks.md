## Implementation Tasks

- [x] Detect the fully drained persistent scheduler state before entering the generic debounce wait path in `src/parallel/orchestration.rs`. (verification: unit - `cargo test -p cflx persistent_idle` passed; targeted scheduler tests verify persistent idle detection and no reconciliation/scan counter increments while no wake event occurs)
- [x] Add an event-driven persistent idle wait path in `src/parallel/orchestration.rs` that wakes on `src/tui/queue.rs` dynamic queue notifications and scheduler-owned wake sources. (verification: unit - `cargo test -p cflx persistent_idle` passed; `test_persistent_idle_wait_wakes_on_queue_push_and_notify_scheduler` covers `DynamicQueue::push` and `notify_scheduler`)
- [x] Preserve normal active-loop behavior for queued work, in-flight work, pending merge results, resolve/reject waiters, and debounce-triggered reanalysis in `src/parallel/orchestration.rs`. (verification: integration - `cargo test -p cflx parallel::tests::executor` passed; normal event select remains separate from persistent fully-idle wait)
- [x] Preserve finite scheduler completion semantics so non-persistent runs still exit when drained. (verification: unit - `cargo test -p cflx test_scheduler_lifetime_controls_idle_exit_behavior` passed)
- [x] Ensure idle observability is useful but non-spamming by emitting the persistent idle message only when entering idle wait, not on every timer tick. (verification: unit - `cargo test -p cflx persistent_idle` passed; idle wait path has no debounce timer branch and `test_persistent_idle_wait_does_not_poll_worktree_reconciliation_without_wake` confirms no repeated scan work)
- [x] Verify the change does not add durable workflow-control state and derives scheduling decisions from existing runtime plus workspace/git evidence. (verification: manual - reviewed `src/parallel/orchestration.rs` and `src/parallel/queue_state.rs`; change adds only runtime helper methods and no durable workflow-control storage, consistent with `openspec/CONSTITUTION.md` laws 1 and 3)
- [x] Run formatting, targeted tests, and the repository's Rust verification commands. (verification: integration - `cargo fmt --check`, `cargo test -p cflx persistent_idle`, `cargo test -p cflx test_scheduler_lifetime_controls_idle_exit_behavior`, `cargo test -p cflx test_queue_notification_triggers_reanalysis`, `cargo test -p cflx parallel::tests::executor`, `agent-exec run -- cargo test -p cflx`, `agent-exec run -- cargo clippy -p cflx --all-targets -- -D warnings`, and `cflx openspec validate fix-persistent-idle-scan-spam --strict` all passed)

## Future Work

- Consider adding lower-level instrumentation counters for scheduler idle transitions if repeated scan regressions become hard to test via existing seams.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-persistent-idle-scan-spam --archive-gate`
