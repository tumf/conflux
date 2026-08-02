## Implementation Tasks

- [x] Define shared application-service methods and typed changed/no-op/failed outcomes for start, retry, stop, cancel stop, force stop, and resolve. (verification: unit - `cargo test --features web-monitoring --lib` verifies service tests cover the complete mode/status matrix and side-effect admission; verification-id: shared-command-tests)

- [x] Replace TUI-specific lifecycle execution paths with thin adapters over the shared services while preserving current warnings, events, scheduling, cancellation, and resolve-queue semantics. (verification: integration - `cargo test --features web-monitoring --lib` verifies TUI adapter tests prove existing successful and invalid-mode behavior through the service boundary; verification-id: shared-command-tests)

- [x] Replace v2 channel-enqueue settlement with shared-service execution and settle command records only after actual acceptance, no-op, or failure plus the resulting synchronous projection revision. (verification: integration - `cargo test --features web-monitoring --lib` verifies command API tests prove no false success, one effect per idempotency key, stale refusal, and settled readback; verification-id: shared-command-tests)

- [x] Connect start to the authoritative marked target set, retry to reconciled routing plus scheduler dispatch, and resolve to single-resolver FIFO scheduling for both live and idle schedulers. (verification: integration - `cargo test --features web-monitoring --lib` verifies table-driven cross-adapter tests compare target IDs, reducer state, scheduler notifications/spawns, and events; verification-id: shared-command-tests)

- [x] Preserve cancellation-first stop-and-dequeue and implement truthful graceful/immediate/force classification and recovery across duplicate, timeout, missing-handle, and completed-work cases. (verification: integration - `cargo test --features web-monitoring --lib` verifies cancellation and force-stop tests prove unsafe cases preserve active state and do not claim force success; verification-id: shared-command-tests)

## Implementation Notes

- The shared run-lifecycle service is `src/orchestration/run_control.rs`, with unit tests in
  `src/orchestration/run_control/tests.rs` over an in-memory reducer, an in-memory queue, and a
  recording scheduler port. No process, repository, network, or timer is involved.
- `src/tui/run_supervisor.rs` implements `RunSchedulerPort` for the local TUI. It owns the
  orchestrator task handle and cancellation token that `src/tui/runner.rs` used to keep in the key
  loop, so a remote start really spawns the run a keypress would have spawned.
- Cross-adapter parity lives in `src/tui/command_handlers/cross_adapter_tests.rs`: one table drives
  every in-scope intent through both `handle_tui_command` and `SharedServiceExecutor` over
  identically arranged harnesses, then compares scheduler calls, reducer display statuses, the mark
  store, and the resolver ledger as a single value. Rows cover valid, invalid-mode, empty-target,
  duplicate, stale-target, scheduler-live, scheduler-idle, and runtime-launch-failure cases.
- `AppState::resolve_merge` is now presentation only. Reservation, FIFO ordering, duplicate
  rejection, the reducer transition, and scheduler dispatch moved into `RunControlService`, so the
  `AppState` tests that asserted those effects were rewritten against the new split rather than kept
  as duplicate coverage.
- `AppState::set_execution_marks` was added so a caller that builds the shared services first can
  bind the app to the mark store those services already read; production still wires it the other
  way round from `runner.rs`.

## Acceptance Repair Notes (attempt 1)

- Resolve promotion is now ledger and presentation bookkeeping only. `complete_resolve_lifecycle`
  pops the promoted change, logs it, and paints `resolve pending`; it no longer re-emits
  `TuiCommand::ResolveMerge`. The promoted change already carries reducer-owned `ResolveWait` from
  the admission that queued it, and `ParallelExecutor` syncs `resolve_wait_changes` from that
  reducer state (`src/parallel/queue_state.rs:308,1308,1325`), so the scheduler — not the frontend —
  owns the promoted dispatch and cannot exit while the intent stands. Re-submission could only be
  refused as no longer `merge wait`, which reached the operator as a red status-bar warning for a
  resolve that was in fact proceeding.
- `handle_resolve_completed`, `handle_merge_completed`, and `complete_resolve_lifecycle` therefore
  return `()`. The refusal is now structurally impossible rather than merely unreached. The
  promotion log line changed from "Queueing scheduler retry intent for '<id>' from resolve queue"
  to "Promoted '<id>' from the resolve queue to the active resolver" because the old wording named
  a command that is no longer emitted. No spec or web asset referenced the old text.
- `tui-architecture` still says the TUI "dequeues the next change and start its resolve
  immediately". That remains observably true — `parallel-execution` spec line 2303 already assigns
  the promotion to the scheduler, which retries reducer-owned `ResolveWait` on the same
  merge-completion trigger — so no spec delta was added for the ownership wording.
- `TuiRunSupervisor` now resolves the dispatch target through `requires_parallel_dispatch`: a launch
  with no targets is scheduler-owned (only the resolve dispatch produces one) and always uses the
  parallel orchestrator, because serial startup replaces the shared `OrchestratorState` over the
  empty target list and would erase the `ResolveWait` intent the launch exists to consume while
  still reporting `SchedulerEffect::Started`. Operator-selected work still honours the `p` toggle.
- The `-u/--integrate-upstream` refusal in `start_run` now follows that resolved dispatch instead of
  the raw toggle, so a scheduler-owned resolve is not refused for a serial dispatch it was never
  going to take. Non-empty serial targets are refused exactly as before.
- Files changed beyond the two declared by the findings:
  `src/tui/state/event_handlers/mod.rs` (dispatch arms follow the `()` return of the two handlers
  changed for `acceptance-resolve-promotion-false-refusal`) and `src/tui/state.rs` (three
  `AppState` tests asserted the removed `TuiCommand` return of those same handlers).

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate unify-remote-operator-commands --archive-gate`

- `cargo test --features web-monitoring --lib`: 2758 passed, 0 failed, 8 ignored (attempt-1 repair
  adds 4 tests to the 2754 of the first pass).
- `cargo fmt --check`: clean.
- `cflx openspec validate unify-remote-operator-commands --strict`: passed.
- The shared cargo target dir is contended by a sibling worktree, which overwrites the lib test
  binary between invocations. The result above is from a copy of this worktree's freshly built
  binary (`cargo test --no-run` → copy → run), not from whatever `cargo test` last left on disk.

## Future Work

- UI confirmation and user-facing command feedback remain consumer responsibilities.

## Current Acceptance Follow-up
- attempt: 1
- [x] [acceptance-resolve-promotion-false-refusal] (minor) FIFO resolve promotion re-emits a ResolveMerge command the shared service can only refuse, surfacing a false red refusal warning to the operator while the promoted resolve actually proceeds via the reducer lane | evidence: src/orchestration/run_control.rs:756-768 applies ReducerCommand::ResolveMerge at submission for queued reservations too, so a queued change is already WaitState::ResolveWait, display 'resolve pending' (src/orchestration/state.rs:519), while it waits behind the active resolver; src/tui/state/event_handlers/completion.rs:131-150 complete_resolve_lifecycle pops the promoted change and re-emits TuiCommand::ResolveMerge expecting it to 'reserve cleanly', but src/orchestration/run_control.rs:747-754 refuses any target whose display status is not 'merge wait', so the promoted re-submission always fails with TargetIneligible; src/tui/command_handlers.rs:534-545 renders that refusal as warning_message 'Manual merge-wait retry intent ... was not accepted by scheduler state' shown as red status-bar text (src/tui/render.rs:1611-1615), even though the scheduler delivers the promoted resolve through reducer-owned ResolveWait and cannot exit while it is pending (src/parallel/orchestration.rs:120-133); the completion.rs test merge_completed_drains_resolve_queue asserts the command emission only; no test drives the promoted command through handle_tui_command, so the false refusal is unexercised | required_changes: src/tui/state/event_handlers/completion.rs — Stop re-emitting the vestigial ResolveMerge command at promotion (make complete_resolve_lifecycle ledger/presentation bookkeeping only, since the reducer lane already owns dispatch of the promoted resolve), or otherwise ensure promotion cannot surface a false operator-facing refusal, and update the tests in this file that assert the command emission | verification: src/tui/state/event_handlers/completion.rs — Test that draining the resolve queue after MergeCompleted promotes the next change without producing a refusal warning: no TuiCommand that the shared service must refuse, promoted change keeps its reducer ResolveWait intent and 'resolve pending' display
  finding: {"evidence":["src/orchestration/run_control.rs:756-768 applies ReducerCommand::ResolveMerge at submission for queued reservations too, so a queued change is already WaitState::ResolveWait, display 'resolve pending' (src/orchestration/state.rs:519), while it waits behind the active resolver","src/tui/state/event_handlers/completion.rs:131-150 complete_resolve_lifecycle pops the promoted change and re-emits TuiCommand::ResolveMerge expecting it to 'reserve cleanly', but src/orchestration/run_control.rs:747-754 refuses any target whose display status is not 'merge wait', so the promoted re-submission always fails with TargetIneligible","src/tui/command_handlers.rs:534-545 renders that refusal as warning_message 'Manual merge-wait retry intent ... was not accepted by scheduler state' shown as red status-bar text (src/tui/render.rs:1611-1615), even though the scheduler delivers the promoted resolve through reducer-owned ResolveWait and cannot exit while it is pending (src/parallel/orchestration.rs:120-133)","the completion.rs test merge_completed_drains_resolve_queue asserts the command emission only; no test drives the promoted command through handle_tui_command, so the false refusal is unexercised"],"id":"acceptance-resolve-promotion-false-refusal","required_changes":[{"description":"Stop re-emitting the vestigial ResolveMerge command at promotion (make complete_resolve_lifecycle ledger/presentation bookkeeping only, since the reducer lane already owns dispatch of the promoted resolve), or otherwise ensure promotion cannot surface a false operator-facing refusal, and update the tests in this file that assert the command emission","file":"src/tui/state/event_handlers/completion.rs"}],"severity":"minor","summary":"FIFO resolve promotion re-emits a ResolveMerge command the shared service can only refuse, surfacing a false red refusal warning to the operator while the promoted resolve actually proceeds via the reducer lane","verification":[{"description":"Test that draining the resolve queue after MergeCompleted promotes the next change without producing a refusal warning: no TuiCommand that the shared service must refuse, promoted change keeps its reducer ResolveWait intent and 'resolve pending' display","file":"src/tui/state/event_handlers/completion.rs"}]}
  evidence: src/tui/state/event_handlers/completion.rs complete_resolve_lifecycle is now ledger/presentation only and returns (), so promotion emits no TuiCommand the shared service could refuse; handle_resolve_completed/handle_merge_completed and the mod.rs dispatch arms follow, and new test merge_completed_promotes_the_queued_resolve_without_a_refusal drives two real RunControlService admissions then MergeCompleted and asserts no command, warning_message None, change-b still "resolve pending" and still the only reducer ResolveWait id (1 passed).
- [x] [acceptance-serial-empty-target-resolve-spawn] (major) With the serial toggle active, an idle-scheduler resolve spawns the serial orchestrator with empty targets, which wipes the shared reducer state and never performs the resolve while the TUI and the v2 command record both report successful dispatch | evidence: src/tui/run_supervisor.rs:113-115 spawn() selects serial vs parallel from the parallel_mode toggle for every start_run, including the empty-target scheduler-owned resolve dispatch reached via src/orchestration/run_control.rs:782-787 and 528-536; src/tui/orchestrator.rs:253-256 run_orchestrator (serial) unconditionally replaces the shared OrchestratorState with OrchestratorState::new over the (empty) target list, destroying the ResolveWait intent recorded at src/orchestration/run_control.rs:758-761 and every other row's runtime state; serial has no equivalent of initialize_parallel_shared_state's preserve_manual_resolve_startup (src/tui/orchestrator.rs:169-189); on main the idle-scheduler resolve unconditionally spawned run_orchestrator_parallel (main src/tui/command_handlers.rs:682-719), so this is a regression introduced by routing resolve dispatch through the toggle-honoring supervisor; reachable from the default flow: merge-wait rows are produced only by the parallel machinery (MergeDeferred emitters src/parallel/merge.rs:487,1140), the run exits to Select, toggle_parallel_mode 'p' is allowed in Select/Stopped (src/tui/key_handlers.rs:801, src/tui/state.rs:998-1011), then M on the merge-wait row logs 'started scheduler for manual resolve' and the v2 record settles succeeded from SchedulerEffect::Started while nothing resolves | required_changes: src/tui/run_supervisor.rs — Make empty-target scheduler-owned runs (manual resolve consumption) spawn the parallel orchestrator regardless of the serial toggle, restoring main's guaranteed run_orchestrator_parallel dispatch, or refuse the launch with an Err so the command settles failed instead of claiming Started while reducer state is wiped | verification: src/tui/run_supervisor.rs — Test that start_run with empty targets and parallel_mode=false selects the parallel orchestrator path (or returns Err), proving a serial-toggle resolve can no longer wipe reducer state and settle as succeeded
  finding: {"evidence":["src/tui/run_supervisor.rs:113-115 spawn() selects serial vs parallel from the parallel_mode toggle for every start_run, including the empty-target scheduler-owned resolve dispatch reached via src/orchestration/run_control.rs:782-787 and 528-536","src/tui/orchestrator.rs:253-256 run_orchestrator (serial) unconditionally replaces the shared OrchestratorState with OrchestratorState::new over the (empty) target list, destroying the ResolveWait intent recorded at src/orchestration/run_control.rs:758-761 and every other row's runtime state; serial has no equivalent of initialize_parallel_shared_state's preserve_manual_resolve_startup (src/tui/orchestrator.rs:169-189)","on main the idle-scheduler resolve unconditionally spawned run_orchestrator_parallel (main src/tui/command_handlers.rs:682-719), so this is a regression introduced by routing resolve dispatch through the toggle-honoring supervisor","reachable from the default flow: merge-wait rows are produced only by the parallel machinery (MergeDeferred emitters src/parallel/merge.rs:487,1140), the run exits to Select, toggle_parallel_mode 'p' is allowed in Select/Stopped (src/tui/key_handlers.rs:801, src/tui/state.rs:998-1011), then M on the merge-wait row logs 'started scheduler for manual resolve' and the v2 record settles succeeded from SchedulerEffect::Started while nothing resolves"],"id":"acceptance-serial-empty-target-resolve-spawn","required_changes":[{"description":"Make empty-target scheduler-owned runs (manual resolve consumption) spawn the parallel orchestrator regardless of the serial toggle, restoring main's guaranteed run_orchestrator_parallel dispatch, or refuse the launch with an Err so the command settles failed instead of claiming Started while reducer state is wiped","file":"src/tui/run_supervisor.rs"}],"severity":"major","summary":"With the serial toggle active, an idle-scheduler resolve spawns the serial orchestrator with empty targets, which wipes the shared reducer state and never performs the resolve while the TUI and the v2 command record both report successful dispatch","verification":[{"description":"Test that start_run with empty targets and parallel_mode=false selects the parallel orchestrator path (or returns Err), proving a serial-toggle resolve can no longer wipe reducer state and settle as succeeded","file":"src/tui/run_supervisor.rs"}]}
  evidence: src/tui/run_supervisor.rs requires_parallel_dispatch makes every empty-target (scheduler-owned resolve) launch use run_orchestrator_parallel regardless of the p toggle, and start_run applies the same resolved mode to the -u refusal; 3 new tests pass, including empty_target_launch_dispatches_parallel_despite_serial_toggle which asserts the parallel startup event with parallel_mode=false and that the reducer ResolveWait intent survives the launch.
