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

## Acceptance Repair Notes (attempt 2)

**acceptance-bin-dead-code-clippy-gate**

- Every symbol the finding named is gone from the crate rather than silenced with an `allow`, except
  the three `AppState` accessors whose only real callers are tests; those are `#[cfg(test)]` now.
- `ControlCommand`, `WebState::control_tx`, `set_control_channel`, and `send_control_command` are
  removed outright. Nothing constructed a lifecycle variant any more: the legacy `/api/control/*`
  routes that used to produce them were removed by this change, and `/api/v2` executes through
  `RunControlService`. Keeping the channel would have kept a second, unreachable command path.
- `TuiCommand::Retry` is removed with its match arm. Its arm only called
  `handle_start_processing_command(Vec::new(), ctx)`, and the key path already sends
  `StartProcessing(Vec::new())`, so the four tests that constructed `Retry` now drive the real
  variant and keep asserting explicit-retry routing and scheduler dispatch unchanged.
- `TuiCommandContext` keeps only what `handle_tui_command` reads. `dynamic_queue`,
  `post_archive_action`, and `upstream_runtime` moved to `TuiRunSupervisor`'s launch context when
  the shared service took over dispatch, so the context copies were duplicates, not wiring.
- Files changed beyond those declared by this finding, and why:
  - `src/main.rs`: sole consumer of the removed `ControlCommand` surface. The `cflx run --web`
    bridge task that received it is deleted. It was already inert — nothing sent on that channel —
    so no reachable behavior changes.
  - `src/web/remote_control_api/tests/compatibility_tests.rs`: the "removed route has no side
    effect" test asserted through the deleted channel. It now asserts the projection revision and
    the admitted/settled command-registry sizes are unchanged, which is the surface a mutation
    would actually move today.
  - `src/tui/runner.rs`, `src/tui/command_handlers/cross_adapter_tests.rs`, `src/tui/README.md`:
    call sites and documentation of the removed context fields and the removed `Retry` variant.
- Observed but deliberately left alone: `cflx run`'s post-error branch in `src/main.rs` polls
  `restart_requested`, which only the (already inert) bridge ever set, so an orchestrator error in
  `cflx run` waits for SIGINT/SIGTERM. That hang predates this repair and is unchanged by it; it is
  not one of the open findings, and rewriting `cflx run`'s restart semantics is out of scope here.

**acceptance-auto-deferral-resolve-false-refusal**

- The auto-resumable idle branch of `handle_merge_deferred` no longer emits
  `TuiCommand::ResolveMerge`; it paints `resolve pending` and returns. The reducer has already
  recorded `ResolveWait` for that change before the TUI sees the event
  (`src/orchestration/state.rs` `MergeDeferred` arm), so the row is never `merge wait` when the
  command would be handled and `RunControlService::resolve_merge` could only answer
  `TargetIneligible` — a red status-bar warning for a resolve the scheduler was about to run.
- The stale comment claiming the service "takes the one reservation this change is allowed to hold"
  is replaced with the actual ownership statement, and the log line now reads "awaiting scheduler
  retry" instead of "queued scheduler retry intent", which named a command that is no longer sent.
- `handle_merge_deferred` and `handle_orchestrator_event` return `()` for the same reason the
  attempt-1 promotion repair did: with the last emitting branch gone, an `Option<TuiCommand>` that
  is structurally always `None` is the same vestige one level up. The refusal is now impossible to
  express, not merely unreached.
- Files changed beyond the one declared by this finding, and why:
  - `src/tui/state/event_handlers/mod.rs` and `src/tui/runner.rs`: the dispatcher and its single
    production caller follow the `()` return.
  - `src/tui/state.rs`, `src/tui/state/event_handlers/completion.rs`,
    `src/tui/state/event_handlers/processing.rs`: tests that bound the removed return value.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate unify-remote-operator-commands --archive-gate`

- `cargo test --features web-monitoring --lib`: 2759 passed, 0 failed, 8 ignored (attempt-2 repair
  adds 1 test and removes none from the 2758 of attempt 1).
- `cargo clippy --locked --all-targets --all-features -- -D warnings`: exit 0, zero errors. This is
  the exact command `.pre-commit-config.yaml` runs via prek on every commit, including the archive
  commit.
- `cargo fmt --check`: clean.
- `cflx openspec validate unify-remote-operator-commands --strict`: passed.
- The shared cargo target dir (`/Volumes/.../rust-target/default`) is contended by a sibling
  worktree running `cargo test --no-fail-fast`, which overwrites both check artifacts and the lib
  test binary between invocations; a copy-then-run of the shared binary was observed executing a
  sibling's tests (old test names, different test count). Both results above therefore come from a
  private `CARGO_TARGET_DIR=/tmp/cflx-repair-target`, with `src/main.rs` and `src/lib.rs` touched
  first so the crate is genuinely re-checked from this tree's sources — the clippy log shows
  `Checking cflx v0.6.216 (<this worktree>)`, and the test log lists this repair's new and renamed
  tests.

## Future Work

- UI confirmation and user-facing command feedback remain consumer responsibilities.
