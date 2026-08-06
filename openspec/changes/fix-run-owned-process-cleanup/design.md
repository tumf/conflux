## Context

Conflux already isolates commands in owned process groups and can verify quiescence after a specific command finishes. The gap is one layer higher: an orchestration run can lose every caller-side handle while `AiCommandRunner`'s detached retry task and process group remain alive.

Three shutdown paths expose the same ownership defect:

1. global cancellation races a 100ms per-change monitor against immediate workspace-future abort;
2. run-fatal handling aborts workspace futures without sending any cancellation to in-flight AI runners;
3. local TUI exit can abort the orchestrator after five seconds, before the command cleanup path can finish.

`DynamicQueue::release_all_execution_handles` currently removes handles and fires `done`, although its own termination contract says `done` proves that the task and child process exited. The run has task-registration evidence but no run-wide runner-task or process-group barrier.

## Goals

- Give one orchestration invocation complete ownership of all AI runner tasks and owned process identities it launches.
- Close spawn and retry admission atomically when global cancellation or run-fatal shutdown begins.
- Preserve prompt run-fatal Error reporting while delaying only terminal stop or scheduler failure return until cleanup.
- Make execution completion handshakes represent actual task and process cleanup.
- Let local TUI shutdown force-clean run-owned process groups even after the orchestrator task stops cooperating.
- Reuse existing process-group cleanup and keep all ownership state ephemeral.

## Non-Goals

- Redesign process-group signal mechanics.
- Introduce durable PID or workflow state.
- Add operation-specific cleanup parameters throughout Apply, Archive, Acceptance, conflict, rejection, or upstream APIs.
- Change remote TUI shutdown semantics.
- Change reducer state reconciliation after `Stopped`.

## Decision: Invocation-Scoped Run Command Scope

Add a clone-shared scope at the command-runner layer. One scope is created for each `cflx run` invocation and each TUI-supervised run start. A restarted TUI run receives a new open scope; a closed scope is never reused.

The scope state contains:

- an open/closing admission state;
- a cancellation token observed directly by runner tasks;
- active execution registrations keyed by an opaque execution ID and carrying operation/change context;
- the current platform process identity for each registration when one exists;
- task-finished and typed process-group cleanup evidence;
- a notification used by the owner to await quiescence.

Use existing Tokio synchronization primitives already in the dependency graph. Do not enable or add `tokio_util::task::TaskTracker`: its `close()` operation does not reject new task spawns, so it cannot provide the required admission boundary by itself.

## Admission and Registration State Machine

An execution follows these states:

1. `Registered`: `execute_streaming_with_retry` atomically reserves an execution before spawning its detached runner task.
2. `WaitingToSpawn`: stagger or retry delay may run, but final process admission remains pending.
3. `Running(process_identity)`: final scope admission and `Command::spawn` occur in one serialized critical section; the process identity is retained immediately.
4. `Cleaning(process_identity)`: shutdown, timeout, natural completion, or retry cleanup is in progress. The identity remains registered.
5. `Quiescent`: typed cleanup confirms no owned process-set members remain and the runner task is ending; only now may the registration disappear and any corresponding `done` handshake fire.

Closing the scope atomically prevents transition from `WaitingToSpawn` to `Running`. A check before stagger or retry sleep is insufficient because shutdown can occur during that wait.

Scope cancellation is the primary shutdown signal. Cancellation-channel closure caused by a dropped `StreamingChildHandle` is treated as cancellation-compatible hardening, never as permission to detach and continue. Every inactivity and ordinary retry branch checks scope state immediately before retry delay and again at final spawn admission.

## Construction and Wiring

The outer run owner creates the scope before constructing run command runners:

- direct CLI orchestration creates it at run start;
- `TuiRunSupervisor::spawn` creates it and retains one clone beside the task handle and cancellation token;
- `ParallelRunService` and `ParallelExecutor` receive the same scoped command runtime;
- the service analyzer and executor share the scoped `AiCommandRunner`;
- Apply, Archive, Acceptance, cleanup review, and rejection review continue receiving that runner;
- conflict and upstream repair reuse the invocation runner or a clone carrying the same scope instead of constructing a runner from only `SharedStaggerState`.

This is construction-boundary plumbing, not operation-boundary plumbing. Operation functions do not gain a cleanup-scope parameter.

The TUI's standalone worktree-command runner is outside a scheduler run and is not silently attached to a later run scope. Its existing caller-owned lifecycle remains unchanged.

## Shutdown and Barrier Ordering

### Global cancellation

1. The run cancellation token closes scope admission and broadcasts runner shutdown.
2. The scheduler stops dispatch and aborts workspace futures.
3. Detached runner tasks clean their current process groups and cannot retry.
4. The scheduler awaits active workspace drain, run command scope quiescence, and pending merge/base-lane handling under the outer cleanup bound.
5. Preparation/workspace release and truthful `done` handshakes occur only after their command registrations are quiescent.
6. The scheduler emits `Stopped`; operator cancellation remains cancellation even if managed escalation was required.

The scope observes the global token directly, so inline dependency analysis and other callers blocked on output or child completion do not need to return to the scheduler loop before cleanup begins.

### Run-fatal failure

1. The typed queue boundary publishes exactly one global `Error` immediately and records run-fatal abort.
2. The same boundary closes scope admission and broadcasts shutdown without awaiting it.
3. The scheduler stops dispatch, aborts workspace futures, and awaits the cleanup barrier.
4. Only after the barrier does the scheduler return its original run-fatal error.

The global `Error` remains before the drain for prompt operator feedback. The barrier is before failure return, not before the Error event. No second global Error, terminal `Stopped`, or `AllCompleted` is emitted.

## Deadline Model

Use one absolute cancellation-start timestamp so nested waits cannot each consume a fresh full budget.

The implementation should define a 30-second run-command cleanup deadline. It exceeds the current command termination plus process-group verification path and assumes active command cleanups run concurrently. The existing 90-second pending merge/base-lane drain runs concurrently with, or consumes the same absolute cancellation budget as, command cleanup. Both remain inside the existing 120-second outer scheduler cleanup boundary.

Local TUI shutdown must no longer use a five-second grace that expires before these layers. It waits through the scheduler's outer cleanup boundary. If that boundary expires, the supervisor still owns the scope, applies final forceful cleanup and verification to retained process identities, then aborts and joins the orchestrator task. Timeout outcome remains distinguishable from graceful completion.

If cleanup cannot be proven because the operating system rejects signaling or verification, the run retains cancellation or run-fatal classification, emits bounded actionable diagnostics, and never labels an unconfirmed `done` handshake as successful process exit.

## Execution-Handle Truthfulness

`JoinSet::abort_all` proves only that workspace futures were dropped. `release_all_execution_handles` must therefore separate registry removal from completion acknowledgement.

For each change:

- confirmed scope cleanup may fire `done` and remove the stale per-change handle;
- unconfirmed cleanup must not fire `done`;
- the retained run-scope process identity remains available for managed escalation and diagnostics;
- per-change waiters time out truthfully rather than receiving false completion.

## Verification Strategy

Fast tests remain below the repository's one-second default-test limit:

- `run_command_scope_refuses_spawn_after_shutdown` closes the scope while a command waits at final admission and proves the command body never runs;
- `run_command_scope_suppresses_retry_after_shutdown` cancels between attempts and proves the attempt counter does not advance;
- `run_fatal_error_precedes_cleanup_barrier` holds a fake registration open, proves one Error arrives promptly, then proves scheduler failure waits for release;
- `execution_done_requires_process_quiescence` aborts a fake workspace future and proves `done` stays pending until the matching scope registration reports confirmed cleanup.

Heavy Unix tests extend `tests/process_cleanup_test.rs` with real process groups and SIGTERM-immune descendants:

- global cancellation has no group member when `Stopped` is emitted;
- run-fatal has no group member when the scheduler returns failure while its Error was emitted earlier exactly once;
- local TUI timeout cleanup has no group member when `AbortedAfterTimeout` returns.

Each heavy test records the PGID before triggering shutdown and probes `killpg(pgid, 0)` after the asserted terminal boundary. Tests must clean up their own process group on assertion setup failure.

## Risks and Mitigations

- **Scope misses an operation-specific runner:** centralize runner construction and add a construction test covering every production run command surface.
- **Shutdown races final spawn:** serialize final scope admission and `Command::spawn`; do not rely on an earlier atomic flag read.
- **One stuck command serializes all cleanup:** cancel all registrations first and await them concurrently under one absolute deadline.
- **Error feedback becomes slow:** preserve immediate queue-owned run-fatal Error publication and wait only before failure return.
- **TUI exit becomes unbounded:** retain fixed nested deadlines and a final supervisor-owned escalation path.
- **Cleanup state influences restart routing:** discard the scope on process/run end and continue deriving next action from workspace and Git evidence only.
- **Active force-stop reconciliation overlaps:** keep this change limited to process ownership and terminal timing; reducer presentation remains owned by `fix-force-stop-reducer-reconciliation`.
