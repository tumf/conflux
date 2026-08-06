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
    requirement: "A transient reducer snapshot write lock cannot convert queued work into an indefinitely idle scheduler, and incomplete evidence never authorizes analysis or dispatch"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Deterministic Tokio test output covering held-write-lock release, coherent classification, automatic continuation, no premature analyzer/dispatch, and unchanged event-driven stable idle behavior"
    rerun: "cargo test --lib reducer_snapshot_contention && cargo test --lib persistent_idle && cargo test --lib reanalysis_trigger_lifetime && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Retry transient queue classification

**Change Type**: implementation

## Problem / Context

Queue classification builds reducer-dependent eligibility in two separate non-blocking `try_read` operations: one in `DependencyContext` and another for queued/wait sets. If either read races with a short reducer write, Conflux fails closed by marking candidates unavailable. That is safe for the current dispatch attempt.

The scheduler then treats the temporary result as stable blocked-only work. In persistent mode it can enter the event-driven idle wait, which intentionally has no timer. A transient lock collision therefore consumes the current evaluation and leaves queued intent idle indefinitely unless an unrelated queue, merge, or cancellation event arrives.

The observed run retained `queue_intent: queued` for more than fourteen hours after one reducer lock collision. No external state changed; the promised next classification pass was never scheduled.

## Proposed Solution

Make queue classification consume one coherent reducer snapshot. Acquire the Tokio reducer read lock asynchronously, copy the queue intent, wait sets, terminal/error state, active/resolving state, and blocker-held sets needed by queue and dependency classification, then release the guard before repository or dependency awaits.

A writer holding the reducer lock temporarily may suspend the classification future, but it must not produce a stable `candidate_unavailable` classification. When the writer releases, the same scheduler evaluation continues automatically without requiring a new queue notification.

Incomplete reducer evidence MUST remain fail-closed: dependency analysis and dispatch cannot run before a coherent snapshot exists. Preserve cancellation responsiveness and avoid holding any reducer guard across repository I/O, VCS calls, dependency analysis, or agent execution.

Retain event-driven persistent idle for genuinely drained or stable blocked-only states. Do not add periodic worktree reconciliation or an analyzer polling loop as the liveness mechanism.

## Atomic Scope Rationale

The snapshot acquisition, shared queue/dependency view, scheduler continuation, and contention regression tests form one liveness guarantee. A snapshot-only refactor without scheduler-path verification could still translate a temporary condition into persistent idle; a wake-only patch would retain inconsistent double reads.

## Acceptance Criteria

1. Queue and dependency classification use one coherent reducer snapshot for a scheduler evaluation.
2. A temporary reducer writer cannot cause queued work to be classified as stable unavailable, blocked-only, drained, or eligible for indefinite persistent idle.
3. Releasing the writer automatically resumes the same classification pass without a queue mutation or external wake notification.
4. No dependency analyzer, worktree preparation, or ordinary dispatch begins while reducer evidence is incomplete.
5. Once the snapshot is available, an ordinary queued candidate proceeds through normal analysis/dispatch and a real held candidate proceeds through stable blocked-only handling.
6. The reducer read guard is released before any repository, VCS, analyzer, dispatch, or other potentially long await.
7. Cancellation remains able to terminate scheduler work while snapshot acquisition is pending.
8. Stable persistent idle remains event-driven and does not resume periodic worktree scans or repeated dependency analysis.
9. Snapshot and retry state remain process-local and do not become durable workflow-control inputs.

## Explicit Completion Conditions

- `src/parallel/queue_state.rs` and `src/parallel/dependency.rs` share a single captured reducer classification view rather than independently treating `try_read` failure as lifecycle evidence.
- A deterministic Tokio regression holds the reducer write guard, proves analysis/dispatch has not started, releases the guard, and proves the queued candidate is evaluated without sending a queue notification.
- A second regression proves the resumed evaluation honors an actual external/Acceptance hold instead of dispatching it.
- Scheduler-path coverage proves transient contention cannot enter `wait_for_persistent_idle_wake`, while existing stable idle tests continue to prove no polling without a real wake.
- Tests prove the reducer guard is not retained across the analyzer or repository-facing portion of classification.
- The commands declared by `queue-classification-liveness` pass.

## Out of Scope

- Reintroducing 500 ms worktree reconciliation while the scheduler is stably idle.
- Re-running the dependency analyzer repeatedly for unchanged completed input.
- Changing queue admission, dependency semantics, blocker classification, or scheduler lifetime modes.
- Persisting reducer snapshots or retry deadlines outside process memory.
- Treating unreadable reducer evidence as permission to dispatch.
