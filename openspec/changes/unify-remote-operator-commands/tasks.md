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

## Current Acceptance Follow-up
- attempt: 3
- [x] [acceptance-auto-deferral-resolve-false-refusal] (minor) Every auto-resumable merge deferral that arrives while the resolver ledger is idle emits a ResolveMerge command the shared service deterministically refuses, surfacing a false red refusal warning while the resolve actually proceeds via the reducer lane | evidence: src/tui/runner.rs:90,777-788 applies MergeDeferred to the shared reducer and syncs display caches before handle_orchestrator_event runs at src/tui/runner.rs:822; src/orchestration/state.rs:2002-2011: auto_resumable=true sets WaitState::ResolveWait (display 'resolve pending') and enqueues reducer-owned resolve-wait intent, so the display is never 'merge wait' when the emitted command is handled; src/tui/state/event_handlers/errors.rs:235-252: the idle branch paints 'resolve pending' and emits Some(TuiCommand::ResolveMerge); its comment claims the service 'takes the one reservation this change is allowed to hold', but the service refuses before any reservation exists; src/orchestration/run_control.rs:748-756 refuses any target whose display is not 'merge wait' with TargetIneligible, and src/tui/command_handlers.rs:534-544 renders that as warning_message 'Manual merge-wait retry intent … was not accepted by scheduler state' (red status bar); auto-resumable deferrals are emitted in the default parallel flow (src/parallel/merge.rs:157,1143) while no manual resolver is active, so the idle branch is the common case; on main the same emission was admitted unconditionally (main src/tui/command_handlers.rs:682-719 dispatched without an eligibility refusal), so the false refusal is introduced by this change; src/tui/state/event_handlers/errors.rs:520-561: auto_resumable_merge_deferred_shows_resolve_wait_not_merge_wait and auto_resumable_merge_deferred_starts_resolve_when_idle assert only the command emission; no test drives it through handle_tui_command, so the refusal is unexercised | required_changes: src/tui/state/event_handlers/errors.rs — Stop emitting the vestigial TuiCommand::ResolveMerge on the auto-resumable idle branch (the reducer lane already owns dispatch and the emitting scheduler cannot exit while ResolveWait stands — same rationale as the accepted promotion repair), correct the stale reservation comment, and update the two tests that assert the emission | verification: src/tui/state/event_handlers/errors.rs — Test that MergeDeferred(auto_resumable=true) applied to the reducer then handled with an idle resolver ledger yields no TuiCommand and no warning_message while the reducer retains the ResolveWait intent and 'resolve pending' display
  finding: {"evidence":["src/tui/runner.rs:90,777-788 applies MergeDeferred to the shared reducer and syncs display caches before handle_orchestrator_event runs at src/tui/runner.rs:822","src/orchestration/state.rs:2002-2011: auto_resumable=true sets WaitState::ResolveWait (display 'resolve pending') and enqueues reducer-owned resolve-wait intent, so the display is never 'merge wait' when the emitted command is handled","src/tui/state/event_handlers/errors.rs:235-252: the idle branch paints 'resolve pending' and emits Some(TuiCommand::ResolveMerge); its comment claims the service 'takes the one reservation this change is allowed to hold', but the service refuses before any reservation exists","src/orchestration/run_control.rs:748-756 refuses any target whose display is not 'merge wait' with TargetIneligible, and src/tui/command_handlers.rs:534-544 renders that as warning_message 'Manual merge-wait retry intent … was not accepted by scheduler state' (red status bar)","auto-resumable deferrals are emitted in the default parallel flow (src/parallel/merge.rs:157,1143) while no manual resolver is active, so the idle branch is the common case; on main the same emission was admitted unconditionally (main src/tui/command_handlers.rs:682-719 dispatched without an eligibility refusal), so the false refusal is introduced by this change","src/tui/state/event_handlers/errors.rs:520-561: auto_resumable_merge_deferred_shows_resolve_wait_not_merge_wait and auto_resumable_merge_deferred_starts_resolve_when_idle assert only the command emission; no test drives it through handle_tui_command, so the refusal is unexercised"],"id":"acceptance-auto-deferral-resolve-false-refusal","required_changes":[{"description":"Stop emitting the vestigial TuiCommand::ResolveMerge on the auto-resumable idle branch (the reducer lane already owns dispatch and the emitting scheduler cannot exit while ResolveWait stands — same rationale as the accepted promotion repair), correct the stale reservation comment, and update the two tests that assert the emission","file":"src/tui/state/event_handlers/errors.rs"}],"severity":"minor","summary":"Every auto-resumable merge deferral that arrives while the resolver ledger is idle emits a ResolveMerge command the shared service deterministically refuses, surfacing a false red refusal warning while the resolve actually proceeds via the reducer lane","verification":[{"description":"Test that MergeDeferred(auto_resumable=true) applied to the reducer then handled with an idle resolver ledger yields no TuiCommand and no warning_message while the reducer retains the ResolveWait intent and 'resolve pending' display","file":"src/tui/state/event_handlers/errors.rs"}]}
  evidence: idle auto-resumable branch in src/tui/state/event_handlers/errors.rs now paints 'resolve pending' and returns without TuiCommand::ResolveMerge, stale reservation comment replaced with the reducer-owned-dispatch rationale, handle_merge_deferred and handle_orchestrator_event return (), the two emission-asserting tests now assert no warning_message, and new unit test auto_resumable_deferral_through_the_reducer_neither_commands_nor_warns drives MergeDeferred(auto_resumable=true) through the reducer with an idle ledger and asserts no warning, 'resolve pending', and retained ResolveWait intent (private-target `cargo test --features web-monitoring --lib`: 2759 passed, 0 failed)
- [x] [acceptance-bin-dead-code-clippy-gate] (major) Change-introduced symbols are dead code in the cflx bin compilation unit, so the pre-commit clippy gate (-D warnings) that runs on every commit, including the final archive commit, fails with 8 errors | evidence: cargo clippy --locked --all-targets --all-features -- -D warnings exits 101 while 'Checking cflx v0.6.216 (/Users/tumf/.local/share/cflx/worktrees/conflux-bda270b8/unify-remote-operator-commands)' with 'could not compile cflx (bin "cflx") due to 8 previous errors'; .pre-commit-config.yaml's clippy hook runs exactly this command via prek, and src/hooks.rs git_commit_no_verify defaults to false, so the archive commit path cannot succeed; src/orchestration/run_control.rs:409 StartEligibility::parallel_mode is never used; src/orchestration/run_control.rs:504 RunControlService::resolves and ::eligibility are never used; src/tui/command_handlers.rs:157 TuiCommandContext fields dynamic_queue, post_archive_action, upstream_runtime are never read; src/tui/events.rs:60 TuiCommand::Retry is never constructed; src/tui/run_supervisor.rs:92 TuiRunSupervisor::parallel_mode is never used; src/tui/state.rs:444 AppState::set_execution_marks, ::set_resolve_reservations, ::queued_resolves are never used in the bin unit (their only callers are cfg(test) code); src/web/state.rs:25 ControlCommand variants Start, Stop, CancelStop, ForceStop are never constructed and src/web/state.rs:532 send_control_command is never used — the channel-enqueue surface this change obsoleted was left in place as dead code; a plain cargo clippy run in this worktree can falsely pass by reusing a sibling worktree's check artifacts from the shared cargo target dir; the errors reproduce whenever the crate is genuinely re-checked from this tree's sources | required_changes: src/web/state.rs — Remove the obsoleted control-channel surface (ControlCommand lifecycle variants and send_control_command) or scope any part that must remain to its real consumer; src/tui/events.rs — Remove the never-constructed TuiCommand::Retry variant (and its match arms) or scope it to its real consumer; src/tui/command_handlers.rs — Drop the never-read TuiCommandContext fields (dynamic_queue, post_archive_action, upstream_runtime) and their initializers, or wire them to real uses; src/orchestration/run_control.rs — Remove or cfg(test)-scope the unused parallel_mode, resolves, and eligibility accessors; src/tui/run_supervisor.rs — Remove or cfg(test)-scope the unused parallel_mode accessor; src/tui/state.rs — Scope set_execution_marks, set_resolve_reservations, and queued_resolves to cfg(test) or their real callers so the bin unit does not see them as dead | verification: src/web/state.rs — cargo clippy --locked --all-targets --all-features -- -D warnings exits 0 from a target dir where the crate is genuinely re-checked (no sibling-worktree artifact reuse), with zero dead_code errors for the cflx bin target
  finding: {"evidence":["cargo clippy --locked --all-targets --all-features -- -D warnings exits 101 while 'Checking cflx v0.6.216 (/Users/tumf/.local/share/cflx/worktrees/conflux-bda270b8/unify-remote-operator-commands)' with 'could not compile cflx (bin \"cflx\") due to 8 previous errors'; .pre-commit-config.yaml's clippy hook runs exactly this command via prek, and src/hooks.rs git_commit_no_verify defaults to false, so the archive commit path cannot succeed","src/orchestration/run_control.rs:409 StartEligibility::parallel_mode is never used; src/orchestration/run_control.rs:504 RunControlService::resolves and ::eligibility are never used","src/tui/command_handlers.rs:157 TuiCommandContext fields dynamic_queue, post_archive_action, upstream_runtime are never read","src/tui/events.rs:60 TuiCommand::Retry is never constructed; src/tui/run_supervisor.rs:92 TuiRunSupervisor::parallel_mode is never used","src/tui/state.rs:444 AppState::set_execution_marks, ::set_resolve_reservations, ::queued_resolves are never used in the bin unit (their only callers are cfg(test) code)","src/web/state.rs:25 ControlCommand variants Start, Stop, CancelStop, ForceStop are never constructed and src/web/state.rs:532 send_control_command is never used — the channel-enqueue surface this change obsoleted was left in place as dead code","a plain cargo clippy run in this worktree can falsely pass by reusing a sibling worktree's check artifacts from the shared cargo target dir; the errors reproduce whenever the crate is genuinely re-checked from this tree's sources"],"id":"acceptance-bin-dead-code-clippy-gate","required_changes":[{"description":"Remove the obsoleted control-channel surface (ControlCommand lifecycle variants and send_control_command) or scope any part that must remain to its real consumer","file":"src/web/state.rs"},{"description":"Remove the never-constructed TuiCommand::Retry variant (and its match arms) or scope it to its real consumer","file":"src/tui/events.rs"},{"description":"Drop the never-read TuiCommandContext fields (dynamic_queue, post_archive_action, upstream_runtime) and their initializers, or wire them to real uses","file":"src/tui/command_handlers.rs"},{"description":"Remove or cfg(test)-scope the unused parallel_mode, resolves, and eligibility accessors","file":"src/orchestration/run_control.rs"},{"description":"Remove or cfg(test)-scope the unused parallel_mode accessor","file":"src/tui/run_supervisor.rs"},{"description":"Scope set_execution_marks, set_resolve_reservations, and queued_resolves to cfg(test) or their real callers so the bin unit does not see them as dead","file":"src/tui/state.rs"}],"severity":"major","summary":"Change-introduced symbols are dead code in the cflx bin compilation unit, so the pre-commit clippy gate (-D warnings) that runs on every commit, including the final archive commit, fails with 8 errors","verification":[{"description":"cargo clippy --locked --all-targets --all-features -- -D warnings exits 0 from a target dir where the crate is genuinely re-checked (no sibling-worktree artifact reuse), with zero dead_code errors for the cflx bin target","file":"src/web/state.rs"}]}
  evidence: removed ControlCommand/control_tx/set_control_channel/send_control_command (src/web/state.rs) with its only consumer, the src/main.rs `cflx run --web` bridge; removed TuiCommand::Retry and its arm (src/tui/events.rs, src/tui/command_handlers.rs) with tests moved to the real StartProcessing path; dropped the three unread TuiCommandContext fields and every initializer; removed StartEligibility::parallel_mode, RunControlService::resolves and ::eligibility (src/orchestration/run_control.rs) and TuiRunSupervisor::parallel_mode (src/tui/run_supervisor.rs); cfg(test)-scoped AppState::set_execution_marks, ::set_resolve_reservations, ::queued_resolves (src/tui/state.rs) — `CARGO_TARGET_DIR=/tmp/cflx-repair-target cargo clippy --locked --all-targets --all-features -- -D warnings` after touching src/main.rs and src/lib.rs exits 0 with zero errors and logs `Checking cflx v0.6.216 (<this worktree>)`
