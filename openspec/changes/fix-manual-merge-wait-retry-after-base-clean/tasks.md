## Implementation Tasks

- [ ] Preserve dirty-base manual deferral semantics in reducer and TUI state handling. (verification: unit - reducer/TUI tests in `src/orchestration/state.rs` and `src/tui/state/event_handlers/errors.rs` cover dirty-base manual deferral returning to `merge wait`; completion: `MergeDeferred(auto_resumable=false)` leaves the change visible as `merge wait`, removes reducer-owned resolve-wait membership, and does not requeue ordinary apply work.)

- [ ] Ensure explicit `M` retry after manual blocker cleanup is accepted as reducer-owned `ResolveWait`. (verification: unit - add or update tests in `src/orchestration/state.rs` for `MergeDeferred(false)` followed by explicit `ResolveMerge`; completion: `ReducerCommand::ResolveMerge(change_id)` transitions a manual `MergeWait` archive-complete, not-yet-merged change into `ResolveWait` and exposes it through `resolve_wait_change_ids()`.)

- [ ] Wake and run lane-wait retry evaluation when the scheduler is already alive. (verification: integration - add or update TUI command handler / parallel executor test coverage proving a live scheduler consumes `ResolveWait` created by manual retry; completion: after `src/tui/command_handlers.rs` logs `Scheduled merge-wait retry intent ...; notified existing scheduler`, the scheduler evaluates reducer-owned base-lane waiters even if the ordinary queued list is empty.)

- [ ] Start and run lane-wait retry evaluation when the scheduler was stopped. (verification: integration - add or update tests around `src/tui/command_handlers.rs` or `src/parallel_run_service.rs` showing scheduler startup with empty normal queue still consumes reducer-owned `ResolveWait`; completion: manual retry with no running scheduler starts `run_orchestrator_parallel(Vec::new(), ...)` using the same shared reducer that accepted `ResolveMerge`, and the executor observes the pending lane waiter.)

- [ ] Prevent stale executor-local retry dedupe from suppressing explicit manual retries. (verification: unit - add or update `src/parallel/tests/executor.rs` coverage for dirty-base deferral, base cleanup, explicit `ResolveMerge`, and retry dispatch reaching merge attempt; completion: a previous dirty-base retry that returned to `merge wait` cannot leave `last_dispatched_resolve_wait_changes`, `last_resolve_wait_base_dirty`, or related local retry state in a form that makes the next explicit `M` a no-op.)

- [ ] Emit actionable evidence when retry still cannot start. (verification: unit - add or update tests in `src/parallel/tests/executor.rs` or related TUI handler tests asserting at least one diagnostic `ParallelEvent::Log` or TUI log entry for a non-dispatchable accepted retry path; completion: if explicit retry is accepted but merge/resolve still cannot start, logs or events identify whether the blocker is missing workspace, stale workspace path, remaining dirty base, active base-mutating lane, or stale/final reducer state.)

- [ ] Run focused and default verification for the affected Rust paths. (verification: integration - run focused `cargo test` selectors for the changed modules and document any broader `cargo test`/lint/typecheck command results in the apply summary; completion: affected reducer, TUI command handler, and parallel executor tests pass, and any default test taking over one second is optimized or marked heavy per repository policy.)

## Future Work

- Manual reproduction in the interactive TUI may be useful as an additional sanity check, but repository tests are the required acceptance evidence.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-manual-merge-wait-retry-after-base-clean --archive-gate`
