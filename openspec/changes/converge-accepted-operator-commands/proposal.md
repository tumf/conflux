---
change_type: implementation
priority: high
dependencies:
  - synchronize-execution-marks
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/frontend-abstraction/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/changes/archive/2026-07-31-unify-operator-command-execution/
  - openspec/changes/archive/2026-08-03-unify-remote-operator-commands/
  - openspec/changes/synchronize-execution-marks/
  - openspec/changes/fix-force-stop-reducer-reconciliation/
  - src/events.rs
  - src/orchestration/operator_command.rs
  - src/orchestration/run_control.rs
  - src/tui/command_handlers.rs
  - src/tui/run_supervisor.rs
  - src/tui/orchestrator.rs
  - src/tui/runner.rs
  - src/tui/state.rs
  - src/web/state.rs
  - src/web/remote_control_api/commands.rs
  - src/web/remote_control_api/executor.rs
  - src/web/remote_control_api/projection.rs
verifications:
  - id: accepted-operator-command-regressions
    requirement: "Equivalent accepted TUI and v2 start, retry, resolve, stop, cancel-stop, force-stop, queue, mark, and dequeue commands produce one coherent process-local effect, one frontend fan-out, truthful scheduler or cancellation behavior, and no partial state on failure"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Non-empty Rust test selection and passing output covering shared application transaction ordering, process mode transitions, scheduler activation, resolve reservations, launch failure rollback, targeted mark deltas, TUI next-frame convergence, and TUI/v2 parity"
    rerun: 'for filter in accepted_operator_command_transaction accepted_operator_command_mode_matrix accepted_operator_command_scheduler_order accepted_operator_command_tui_convergence; do cargo test --features web-monitoring "$filter" -- --list | grep -q ": test$" || exit 1; done && cargo test --features web-monitoring accepted_operator_command && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings'
    prerequisites:
      - execution-mark-event-regressions
    execution_class: repository-local
    completion_role: change-blocking
  - id: accepted-command-revision-regressions
    requirement: "Each new v2 command is revision-fenced through completion, stores the exact revision produced by its synchronous accepted effect, and preserves the original record under replay without duplicate side effects"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Passing remote-control integration tests covering concurrent same-revision submissions, success/no-op/failure result revisions, later scheduler progress, exact replay, and idempotency mismatch"
    rerun: 'cargo test --features web-monitoring accepted_command_revision -- --list | grep -q ": test$" && cargo test --features web-monitoring accepted_command_revision'
    prerequisites:
      - accepted-operator-command-regressions
    execution_class: repository-local
    completion_role: change-blocking
---

# Converge accepted operator commands

**Change Type**: implementation

## Premise / Context

- `OperatorCommandService` and `RunControlService` already execute TUI and `/api/v2` operator intent through shared reducer, mark, queue, resolve, scheduler, and cancellation ports.
- TUI command handlers immediately project accepted `RunControlOutcome` values into `AppState`, while `SharedServiceExecutor` only rebuilds the Web snapshot after service execution. The same accepted command can therefore leave the live TUI, Web `app_mode`, and `is_resolving` at different states.
- `/api/v2` command admission reserves a record, spawns execution outside the projection lock, and records whatever revision exists when completion runs. Two new commands can be admitted against one revision, and `result_revision` can omit the command's synchronous effect or include unrelated later progress.
- `RunSchedulerPort::start_run` may let scheduler events race the accepted command projection. A late `RunDispatched` presentation must not overwrite an already delivered terminal or stopping event.
- `ExecutionMarkStore` is the process-local mark authority. The active `synchronize-execution-marks` change owns event-driven mark revocation and TUI store-to-row projection; this change consumes that output and removes remaining command-side full-store replacement.
- Process mode, marks, command coordination, and resolve reservations remain ephemeral. Restart must still derive the next workflow action from workspace and Git evidence under `openspec/CONSTITUTION.md`.

## Problem / Context

The shared command services currently stop at typed service outcomes. Each adapter independently decides how and when to turn those outcomes into frontend mode, row cache, resolve, and API revision changes.

This gap is observable immediately. A remote graceful stop can set the scheduler flag while the API snapshot still reports `running`, so a following `cancel_stop` is rejected against stale projected mode. Active resolve reservation can be accepted while `is_resolving` remains false until a scheduler event. Remote mark, queue, or dequeue changes can update the shared store/reducer without reaching the next TUI render, and a later TUI-wide mark publish can overwrite another frontend's command delta.

The command registry compounds the problem. Admission and execution are separate critical sections. New requests with the same expected revision can both reserve before either effect reaches projection, while completion samples the then-current revision instead of retaining the revision produced by that command. Scheduler launch failure can also occur after retry, queue, mark, or resolve state has already changed, leaving a failed record with partial accepted intent.

## Proposed Solution

Introduce one process-local operator application transaction shared by TUI and v2 adapters. It serializes a new command from final revision/mode revalidation through typed service execution, authoritative outcome dispatch, exact result-revision capture, and scheduler activation or notification.

The transaction SHALL:

1. resolve exact idempotent replay before new-command validation and return the stored record without service execution;
2. serialize each new command against the projection owner, revalidate `expected_revision`, process lifecycle mode, target status, and eligibility immediately before mutation, and prevent a second new command from executing against the consumed revision;
3. prepare scheduler launch or wake without allowing scheduler events to overtake the accepted command effect;
4. commit reducer commands, execution-mark deltas, queue effects, resolve reservations, graceful-stop state, and process-mode transition as one accepted decision;
5. publish one typed operator-command outcome through the process-wide authoritative `EventSink` dispatch boundary so TUI, Web, `/api/v2`, and lifecycle observers consume the same post-transition state;
6. activate or wake scheduling only after the accepted outcome dispatch is visible, while keeping the scheduler free to emit later progress as separate events and revisions;
7. settle the command record with the exact revision returned by that outcome dispatch rather than sampling global revision afterwards;
8. rollback or avoid every staged reducer, mark, queue/retry-edge, resolve-reservation, mode, and scheduler effect when preparation or activation fails.

Use existing execution events when their semantics are exact: graceful stop uses `Stopping`, settled force stop uses `Stopped`, and successful target dequeue uses `ChangeDequeued`. Add one typed operator-outcome event vocabulary only for accepted effects with no existing event, including run dispatch, stop cancellation, force-stop safe-boundary wait, mark/queue delta, and resolve reservation. The reducer remains the owner of change lifecycle; the new event carries process-level decision facts and frontend projection, not a second workflow state machine.

TUI command handlers and `SharedServiceExecutor` become thin adapters over this transaction. TUI `execution_mode`, row `selected`, and Web `app_mode` are projections, not command-admission authority. Command-side TUI mark updates are target-delta based and never rebuild the complete shared store from `ChangeState::selected`.

## Atomic Scope Rationale

Command serialization, outcome fan-out, scheduler ordering, and exact `result_revision` must ship together. Serializing admission without synchronous outcome projection still records stale modes; projecting outcomes after an ungated scheduler spawn still permits terminal-event inversion; capturing a revision without fail-atomic service behavior can certify a partial mutation. These pieces cannot be independently verified as truthful accepted-command semantics.

`synchronize-execution-marks` is a hard dependency because this change consumes its repository output: a common dispatcher bound to `ExecutionMarkStore` and one-way store-to-TUI mark projection. Without that output, command fan-out can reintroduce competing mark ownership. `fix-force-stop-reducer-reconciliation` is not a hard dependency: this proposal emits the existing authoritative `Stopped` event in the correct order, while that independent change may later strengthen the reducer transition consumed by the same event.

## Acceptance Criteria

1. Equivalent TUI and v2 start, resume, retry, resolve, stop, cancel-stop, force-stop, set-mark, set-queue-intent, and stop-and-dequeue intents enter the same application transaction and return equivalent typed changed, no-op, or failed outcomes and errors.
2. Start in Select or Stopped consumes the authoritative marked set at the admitted revision, commits the exact target queue intent, projects `Running`, and activates the scheduler once. No marked/startable target returns no-op or failure without false success or scheduler effect.
3. Error retry uses the existing evidence-aware route, commits marks/queue/retry intent, projects `Running`, and starts or wakes scheduling once. Unsupported or non-resumable retry preserves all blocker evidence and produces no partial side effect.
4. Active resolve reservation projects the reducer status, `is_resolving=true`, and `Running` in the same accepted-command revision before scheduler progress. Queued resolve preserves FIFO and current mode, duplicate reservation is revision-idempotent, and only the active reservation dispatches scheduling.
5. Graceful stop projects `Stopping` in the accepted revision; cancel-stop from that state projects `Running`; invalid modes fail before mutating the graceful-stop flag.
6. Force stop reports the same safe-boundary classification to both adapters. Awaiting cleanup projects `Stopping`; settled cancellation emits authoritative `Stopped`; neither path publishes terminal state before required cleanup.
7. Successful stop-and-dequeue emits `ChangeDequeued`, clears only the target's ordinary queue/mark eligibility, and reaches the next TUI render and v2 snapshot coherently. Missing handles, cancellation failure, or timeout preserve active state.
8. TUI command-side mark and queue projection applies only IDs named by the accepted outcome. A remote delta for an unrelated row cannot be erased by a later local command, and hidden Error retry intent is not converted into a checked row by broad synchronization.
9. Archived and process-level Stopped rows retain execution marks; successful target dequeue/stop clears only that target. Event-driven Error and Rejected revocation remain owned by `synchronize-execution-marks` and are not duplicated here.
10. Every accepted state-changing command produces at most one synchronous state revision containing all of its decision fields. Later scheduler progress may advance revision separately and never rewrites the command record's `result_revision`.
11. A no-op does not advance revision. A failed command has no reducer, mark, queue, retry-edge, resolve-reservation, stop-flag, mode, scheduler, hook, or event side effect and records the unchanged admitted revision.
12. Two new commands submitted with the same expected revision cannot both execute after the first changes state. The later command fails stale without service execution unless it is an exact idempotent replay.
13. Exact replay after any later state advance returns the original command ID, outcome, detail, and `result_revision` without repeating scheduler, cancellation, queue hook, reservation, event, or projection effects. A reused key with a different typed identity remains `idempotency_mismatch`.
14. Scheduler events cannot overtake the accepted command outcome. Late or duplicate command/outcome delivery cannot overwrite Error, Stopping, Stopped, or a later completion state.
15. All coordination state is process-local and discarded at restart; workspace and Git evidence remain the only durable next-action authority.
16. Added default-suite tests remain under one second each or follow the repository heavy-test policy when an existing platform boundary makes that impractical.

## Explicit Completion Conditions

- One application transaction owns final new-command revision/mode validation, shared service invocation, fail-atomic commit, authoritative outcome dispatch, exact revision return, and scheduler activation ordering for both adapters.
- `src/web/remote_control_api/commands.rs` no longer permits two new state-changing commands to execute from one consumed revision, and `Projection::complete_command` receives the command-produced revision explicitly instead of sampling current global state.
- `src/orchestration/run_control.rs` and scheduler supervision expose a prepared/activation boundary or equivalent gate that prevents scheduler event emission before accepted outcome dispatch and leaves no mutation after launch failure.
- `src/events.rs` exhaustively classifies the minimal typed operator outcome vocabulary and sends it through the same process-wide dispatch owner used by orchestration events; no synthetic `ProcessingStarted` or other lifecycle event is used to fake command acceptance.
- `src/tui/command_handlers.rs`, `src/tui/runner.rs`, `src/tui/state.rs`, `src/web/state.rs`, and `src/web/remote_control_api/executor.rs` consume the same outcome projection without maintaining independent admission mode or resolve truth.
- Command-side execution-mark projection uses target deltas only and integrates with the common mark dispatcher supplied by `synchronize-execution-marks`; no command path calls a stale-row full-store replacement.
- Table-driven tests cover the full mode/status matrix, scheduler live/idle and preparation failure, active/queued/duplicate resolve, force-stop safe-boundary classes, dequeue cancellation errors, unrelated mark preservation, TUI next-frame convergence, concurrent expected revisions, no-op/failure revision behavior, and exact replay.
- The commands declared by `accepted-operator-command-regressions` and `accepted-command-revision-regressions` pass with non-empty test selections.

## Out of Scope

- Adding new `/api/v2` command variants, including parallel-mode control.
- Changing bulk-mark target classification, exclusion reasons, or response schema owned by separate bulk/parallel-control work.
- Reimplementing event-driven Error, Rejected, or refresh mark revocation owned by `synchronize-execution-marks`.
- Defining the detailed per-change reducer cleanup performed by `ExecutionEvent::Stopped`; `fix-force-stop-reducer-reconciliation` owns that transition.
- Changing persistent scheduler idle/Ready semantics owned by `restore-ready-on-persistent-idle`.
- Persisting process mode, marks, resolve reservations, command execution locks, or scheduler activation state.
- Changing cancellation deadlines, process-group cleanup mechanics, worktree safety policy, browser controls, or local filesystem escape hatches.
