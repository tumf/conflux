---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - src/parallel/dependency.rs
  - src/parallel/queue_state.rs
  - src/parallel/orchestration.rs
  - src/parallel/tests/executor.rs
  - src/parallel/tests/reanalysis_trigger_lifetime.rs
verifications:
  - id: queue-classification-liveness
    requirement: "Transient reducer lock contention cannot discard queue intent, create false drained or blocked-only state, or terminate/idle the scheduler, and incomplete evidence never authorizes analysis or dispatch"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Deterministic Tokio test output covering contended queue admission/reconciliation/classification, held-write-lock release, finite and persistent scheduler liveness, no premature analyzer/dispatch, and unchanged event-driven stable idle behavior"
    rerun: "cargo test --lib reducer_snapshot_contention && cargo test --lib persistent_idle && cargo test --lib reanalysis_trigger_lifetime && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Retry transient queue classification

**Change Type**: implementation

## Problem / Context

One scheduler pass reads reducer-owned work through several independent non-blocking `try_read` operations. Dynamic queue admission can discard a consumed hint, queue reconciliation can return an empty outcome, and queue/dependency classification can mark existing candidates unavailable. Each local decision fails closed, but together they can erase all scheduler-local evidence of still-queued reducer intent.

The scheduler then treats the temporary result as stable blocked-only or fully drained work. Persistent mode can enter an event-driven idle wait with no timer; finite mode can exit as drained or `BlockedOrStalled`. A transient lock collision can therefore strand or terminate queued work unless an unrelated queue, merge, or cancellation event arrives. The reported incident observed queued reducer intent remaining idle for more than fourteen hours; this proposal treats reducer contention as the source hypothesis confirmed by the matching code path, not as persisted causal proof.

## Proposed Solution

Make reducer-dependent scheduler work detection consume one coherent snapshot per evaluation. Acquire the Tokio reducer read lock asynchronously before dynamic queue admission, queue-intent reconciliation, drain/idle decisions, and queue/dependency classification; copy the queue intent, wait sets, terminal/error state, active/resolving state, and blocker-held sets those stages need; then release the guard before repository or dependency awaits. A consumed dynamic queue hint may be evaluated against this same snapshot or deferred until that awaited snapshot is available, but transient unreadability must not drop its only wake edge.

A writer holding the reducer lock temporarily may suspend the scheduler evaluation, but it must not produce an empty reconciliation, stable `candidate_unavailable`, blocked-only, or drained classification. When the writer releases, the same scheduler evaluation continues automatically without requiring a new queue notification.

Incomplete reducer evidence MUST remain fail-closed: dependency analysis and dispatch cannot run before a coherent snapshot exists. Preserve cancellation responsiveness and avoid holding any reducer guard across repository I/O, VCS calls, dependency analysis, or agent execution.

Retain event-driven persistent idle for genuinely drained or stable blocked-only states. Do not add periodic worktree reconciliation or an analyzer polling loop as the liveness mechanism.

## Atomic Scope Rationale

Snapshot acquisition, dynamic-hint admission, queue reconciliation, queue/dependency classification, scheduler termination/idle decisions, and contention regressions form one liveness guarantee. A classification-only refactor would leave the empty-local-queue drain path broken; a wake-only patch would retain inconsistent reads and lost queue hints.

## Acceptance Criteria

1. Dynamic queue admission, reducer-intent reconciliation, queue/dependency classification, and drain/idle decisions use one coherent reducer snapshot for a scheduler evaluation, or wait for an equivalent coherent snapshot without consuming the only queue edge.
2. A temporary reducer writer cannot cause queued work to be discarded or classified as stable unavailable, blocked-only, drained, or eligible for indefinite persistent idle.
3. Releasing the writer automatically resumes the same scheduler evaluation without a queue mutation or external wake notification.
4. No dependency analyzer, worktree preparation, or ordinary dispatch begins while reducer evidence is incomplete.
5. Once the snapshot is available, an ordinary reducer-queued candidate is reconciled into the scheduler-local queue and proceeds through normal analysis/dispatch; a real held candidate proceeds through stable blocked-only handling.
6. A finite scheduler cannot report `DrainedSuccessfully` or `BlockedOrStalled` solely because reducer evidence was temporarily unreadable.
7. The reducer read guard is released before any repository, VCS, analyzer, dispatch, or other potentially long await.
8. Cancellation remains able to terminate scheduler work while snapshot acquisition is pending.
9. Stable persistent idle remains event-driven and does not resume periodic worktree scans or repeated dependency analysis.
10. Snapshot and retry state remain process-local and do not become durable workflow-control inputs.

## Explicit Completion Conditions

- `src/parallel/queue_state.rs`, `src/parallel/dependency.rs`, and scheduler drain/idle decisions share one captured reducer work view rather than independently treating `try_read` failure as lifecycle evidence.
- Dynamic queue admission and reducer-intent reconciliation no longer consume or erase the only scheduler wake when reducer state is temporarily unreadable.
- A deterministic Tokio regression starts with an empty scheduler-local queue and reducer-visible queued intent, holds the reducer write guard, proves analysis/dispatch has not started, releases the guard, and proves reconciliation plus dispatch proceeds without sending another queue notification.
- A second regression proves the resumed evaluation honors an actual external/Acceptance hold instead of dispatching it.
- Scheduler-path coverage proves transient contention cannot enter `wait_for_persistent_idle_wake` or terminate a finite run as `DrainedSuccessfully`/`BlockedOrStalled`, while existing stable idle tests continue to prove no polling without a real wake.
- Existing tests that hold a write guard while synchronously invoking `DependencyContext` or awaiting classification are replaced with coordinated separate-task contention tests, preventing the new awaited read contract from self-deadlocking.
- Tests prove the reducer guard is not retained across the analyzer or repository-facing portion of classification.
- The commands declared by `queue-classification-liveness` pass.

## Out of Scope

- Reintroducing 500 ms worktree reconciliation while the scheduler is stably idle.
- Re-running the dependency analyzer repeatedly for unchanged completed input.
- Changing queue admission semantics for readable reducer state, dependency semantics, blocker classification, or scheduler lifetime modes.
- Persisting reducer snapshots or retry deadlines outside process memory.
- Treating unreadable reducer evidence as permission to dispatch.
