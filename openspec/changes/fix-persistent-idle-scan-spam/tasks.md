## Implementation Tasks

- [ ] Detect the fully drained persistent scheduler state before entering the generic debounce wait path in `src/parallel/orchestration.rs`. (verification: unit - `cargo test -p cflx persistent_idle` or targeted `src/parallel/tests/executor.rs` scheduler test observes no repeated reconciliation/scan while persistent idle has no wake event)
- [ ] Add an event-driven persistent idle wait path in `src/parallel/orchestration.rs` that wakes on `src/tui/queue.rs` dynamic queue notifications and scheduler-owned wake sources. (verification: unit - `cargo test -p cflx persistent_idle` or targeted `src/parallel/tests/executor.rs` test uses `DynamicQueue::push` or `notify_scheduler` to wake the idle scheduler)
- [ ] Preserve normal active-loop behavior for queued work, in-flight work, pending merge results, resolve/reject waiters, and debounce-triggered reanalysis in `src/parallel/orchestration.rs`. (verification: integration - `cargo test -p cflx parallel::tests::executor` or targeted executor tests covering queued dispatch and merge-result paths)
- [ ] Preserve finite scheduler completion semantics so non-persistent runs still exit when drained. (verification: unit - `cargo test -p cflx test_scheduler_lifetime_controls_idle_exit_behavior` or equivalent targeted test remains passing)
- [ ] Ensure idle observability is useful but non-spamming by emitting the persistent idle message only when entering idle wait, not on every timer tick. (verification: unit/manual - targeted `src/parallel/tests/executor.rs` log/counter assertion where feasible; otherwise run `RUST_LOG=debug cflx ...` and confirm no repeated idle/scan log burst)
- [ ] Verify the change does not add durable workflow-control state and derives scheduling decisions from existing runtime plus workspace/git evidence. (verification: manual - code review `src/parallel/orchestration.rs` and `src/parallel/queue_state.rs` against `openspec/CONSTITUTION.md` laws 1 and 3)
- [ ] Run formatting, targeted tests, and the repository's Rust verification commands. (verification: integration - `cargo fmt --check`, targeted `cargo test -p cflx ...`, and broader `cargo test`/`cargo clippy` or documented blocker evidence)

## Future Work

- Consider adding lower-level instrumentation counters for scheduler idle transitions if repeated scan regressions become hard to test via existing seams.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-persistent-idle-scan-spam --archive-gate`
