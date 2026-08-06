---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/process-execution/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/tui-architecture/spec.md
  - openspec/changes/archive/2026-05-21-fix-tui-exit-cancels-local-agents/
  - openspec/changes/archive/add-strict-process-cleanup/
  - openspec/changes/archive/2026-07-31-fix-idle-parallel-stop-classification/
  - openspec/changes/fix-force-stop-reducer-reconciliation/
  - src/ai_command_runner.rs
  - src/process_manager.rs
  - src/parallel/dispatch.rs
  - src/parallel/orchestration.rs
  - src/parallel/queue_state.rs
  - src/tui/run_supervisor.rs
  - src/tui/orchestrator.rs
  - src/tui/runner.rs
  - src/tui/queue.rs
  - tests/process_cleanup_test.rs
verifications:
  - id: run-owned-process-cleanup-regressions
    requirement: "Every AI command owned by one orchestration invocation closes admission on cancellation or run-fatal failure, suppresses retries, reaches bounded process-group quiescence before terminal stop or scheduler failure return, and remains clean on local TUI timeout escalation"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust unit and heavy Unix integration test output proving atomic admission closure, retry suppression, prompt exactly-once run-fatal Error ordering, truthful execution handshakes, and no surviving owned process-group members after global cancellation, run-fatal failure, or local TUI quit"
    rerun: "cargo test --lib -- --list | grep -q run_command_scope_refuses_spawn_after_shutdown && cargo test --lib -- --list | grep -q run_command_scope_suppresses_retry_after_shutdown && cargo test --lib run_command_scope_ && cargo test --lib -- --list | grep -q run_fatal_error_precedes_cleanup_barrier && cargo test --lib run_fatal_error_precedes_cleanup_barrier && cargo test --lib -- --list | grep -q execution_done_requires_process_quiescence && cargo test --lib execution_done_requires_process_quiescence && cargo test --lib -- --list | grep -q local_tui_shutdown_waits_for_run_command_scope && cargo test --lib local_tui_shutdown_waits_for_run_command_scope && cargo test --features heavy-tests --test process_cleanup_test -- --list | grep -q run_scope_global_cancellation_cleans_process_group && cargo test --features heavy-tests --test process_cleanup_test -- --list | grep -q run_scope_run_fatal_cleans_process_group && cargo test --features heavy-tests --test process_cleanup_test -- --list | grep -q run_scope_tui_quit_cleans_process_group_after_timeout && cargo test --features heavy-tests --test process_cleanup_test run_scope_ && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix run-owned process cleanup

**Change Type**: implementation

## Problem / Context

`AiCommandRunner::execute_streaming_with_retry` owns the real command process in a detached Tokio task, while callers retain only a `StreamingChildHandle`. `StreamingChildHandle::terminate()` sends a cancellation signal but does not wait for the cleanup report or background task. If a workspace future is aborted, dropping the handle currently closes that channel and the detached task deliberately continues, including retrying after its caller and run have ended.

The parallel cancellation and run-fatal exits abort workspace futures and release per-change execution handles, but neither operation proves that runner tasks or their process groups are quiescent. Run-fatal shutdown does not currently signal in-flight AI commands at all. The scheduler may therefore clear preparation state or return failure while an agent descendant still edits a managed worktree. Global cancellation has the same defect as a race between its polling monitor and `JoinSet::abort_all`.

Local TUI shutdown compounds the gap: it cancels the orchestrator, waits five seconds, then aborts the orchestrator task. That grace is shorter than the existing command termination and process-group verification path and shorter than the scheduler's 120-second cancellation boundary. A child placed in its own session can survive the parent task and process exit.

The lower-level SIGTERM, SIGKILL, and process-group verification mechanics are already present and tested. The missing layer is invocation-scoped ownership, admission closure, acknowledgement, and terminal ordering.

## Proposed Solution

Introduce one ephemeral, clone-shared run command scope for each orchestration invocation. The scope owns every `AiCommandRunner` retry task and current process identity launched by that invocation across dependency analysis, Apply, Archive, Acceptance, cleanup review, rejection review, conflict resolution, and upstream repair.

The scope SHALL:

- atomically reject final command spawn admission after shutdown starts, including a shutdown that races with stagger delay or retry delay;
- notify runner tasks directly through a scope cancellation token rather than depending on a droppable `StreamingChildHandle`;
- retain each execution registration and current PGID or platform process-set identity until the retry task has exited and cleanup evidence confirms quiescence or bounded managed escalation completes;
- suppress inactivity and ordinary retry branches after shutdown is observed;
- await all registrations under one bounded absolute deadline, then reuse the existing managed force-kill and verification path for retained process identities;
- remain process-local and be recreated on every new run.

Create and inject the scope only at run construction boundaries. Existing operation methods SHALL reuse the invocation's scoped `AiCommandRunner`; they SHALL NOT each receive new cleanup parameters or construct unscoped runners from the shared stagger timestamp. The final admission check and `Command::spawn` must be serialized with scope shutdown; checking only before stagger or retry sleep is insufficient.

Global cancellation and run-fatal failure both close scope admission and signal active commands before aborting workspace futures. The scheduler then drains workspace tasks, waits for run command scope cleanup and pending merge/base-lane handling under the existing outer bound, and only afterwards establishes terminal `Stopped` or returns run-fatal failure. Conflux-owned worktree cleanup or preparation release must not race a live registered command.

Preserve the run-fatal event contract: the queue boundary emits exactly one global `Error` promptly without waiting for cleanup. The cleanup barrier follows that notification and precedes scheduler failure return; no second global error, `Stopped`, or `AllCompleted` is emitted for the same run-fatal outcome.

Local TUI supervision retains a clone of the active scope outside the spawned orchestrator task. TUI quit begins scope shutdown immediately and uses a shutdown deadline that does not undercut the scheduler or command cleanup budgets. If the orchestrator still times out, the supervisor performs managed forceful cleanup and verification from retained process identities before aborting and joining the task.

Execution-handle `done` signals remain truthful: aborting a workspace future or removing a registry entry is not completion evidence. A waiter observes `done` only after the corresponding run-owned command registration has reached terminal process cleanup; unconfirmed cleanup remains a timeout/diagnostic and retains escalation evidence.

## Atomic Scope Rationale

The command scope, scheduler barriers, execution handshakes, and TUI supervisor escalation must ship together. A scope without terminal barriers can still be dropped before cleanup; barriers without atomic runner admission can miss a retry spawned during shutdown; TUI abort without an externally retained scope can destroy the only path to the PGID. Splitting these changes would create an intermediate state that still reports cleanup without proof.

This proposal has no hard dependency on `fix-force-stop-reducer-reconciliation`: that change consumes the terminal `Stopped` event to reconcile reducer presentation, while this change controls when the already-existing event may truthfully be emitted.

## Acceptance Criteria

1. Every production AI command launched by a parallel run uses the same invocation-scoped command scope, including analyze, Apply, Archive, Acceptance, cleanup review, rejection review, conflict resolve, and upstream repair paths.
2. Scope shutdown atomically closes final spawn admission. A command waiting for stagger, retry delay, or a new retry attempt cannot spawn after closure.
3. Global cancellation reaches a runner task independently of `StreamingChildHandle` lifetime, terminates the current owned process group through the existing cleanup path, and starts no later retry.
4. A dropped streaming handle cannot authorize detached continuation. Its runner remains scope-owned until cleanup and task completion are acknowledged.
5. Global cancellation emits terminal `Stopped` only after active workspace task drain, a truthful execution-handle outcome (confirmed completion or bounded unconfirmed timeout), run command scope quiescence or completed managed escalation, and pending merge/base-lane result handling. Cleanup failure remains classified as operator cancellation and produces actionable diagnostics rather than `AgentCommand` failure.
6. Run-fatal handling emits exactly one prompt global `Error`, closes admission, starts no new dispatch or retry, and returns scheduler failure only after the same run-owned command cleanup barrier. It emits neither `Stopped` nor `AllCompleted` for that failure.
7. No Conflux-owned preparation release, worktree cleanup, handoff, or Git mutation occurs after workspace abort while a command registration for that worktree remains live.
8. Per-change `done` handshakes are not fired merely because `JoinSet::abort_all` dropped a workspace future; they require confirmed terminal command cleanup for the corresponding change.
9. Local TUI quit begins command-scope shutdown at cancellation time. Graceful completion normally joins the orchestrator; timeout escalation force-cleans retained owned process groups before task abort, and deterministic Unix regressions leave no group members behind even for SIGTERM-immune descendants.
10. Cleanup deadlines use one bounded ordering: per-command cleanup fits within the run command scope deadline, command-scope and 90-second merge cleanup do not additively exceed the 120-second outer scheduler boundary, and local TUI shutdown does not expire before that boundary can perform managed cleanup.
11. Existing process-group creation, SIGTERM/SIGKILL verification, run-fatal Error promptness, operator-cancellation classification, and remote TUI client-close semantics remain intact.
12. The ownership registry is process-local only. Restart creates a fresh scope and derives workflow routing from workspace and Git evidence in accordance with `openspec/CONSTITUTION.md`.

## Explicit Completion Conditions

- `src/ai_command_runner.rs` contains one run command scope with atomic admission closure, direct shutdown notification, active execution/process identity registration, retry suppression, bounded quiescence wait, and no dependency on `tokio_util::task::TaskTracker` admission semantics.
- Parallel run construction creates exactly one fresh scope and every production run-owned `AiCommandRunner` shares it; tests enumerate the run command surfaces and fail if any path constructs an unscoped runner.
- `src/parallel/orchestration.rs` begins scope shutdown before workspace abort, does not release preparation or truthful completion handshakes before command cleanup, and awaits command cleanup for both operator cancellation and run-fatal exits.
- `src/parallel/queue_state.rs` retains the single prompt run-fatal global `Error` owner and does not await cleanup before publishing that event.
- `src/tui/run_supervisor.rs`, `src/tui/orchestrator.rs`, and `src/tui/runner.rs` retain and consume the scope across local quit, align bounded deadlines, and perform final managed cleanup before timeout abort.
- Fast regressions prove atomic no-spawn admission, no retry after shutdown, Error-before-barrier ordering, and truthful `done` semantics.
- Heavy Unix regressions in `tests/process_cleanup_test.rs` prove no owned process-group member survives global cancellation, run-fatal failure return, or local TUI timeout abort, including a SIGTERM-immune descendant.
- The command declared by `run-owned-process-cleanup-regressions` passes.

## Out of Scope

- Changing the Apply pre-complete repair watchdog already integrated by `fix-precomplete-apply-repair-termination`.
- Implementing reducer row reconciliation or TUI header presentation covered by active independent changes.
- Replacing the existing Unix process-group or Windows job-object termination implementation.
- Refactoring the per-change 100 ms cancellation polling task in `src/parallel/dispatch.rs`; it owns no agent process, remains a separate task-lifecycle concern, and is not part of run command scope quiescence evidence.
- Persisting process IDs, cleanup registries, cancellation state, or workflow-control state outside the run invocation.
- Killing unrelated processes or descendants that deliberately escape the process group/session owned by Conflux.
- Changing remote TUI client close into a remote server stop command.
- Adding cleanup timeout configuration before an operator requirement demonstrates that fixed internal bounds are insufficient.

Repository-wide Rust format and clippy remain explicitly included in the declared verification because `.pre-commit-config.yaml` selects those hooks only for Rust or Cargo paths; proposal-only commits do not exercise them.
