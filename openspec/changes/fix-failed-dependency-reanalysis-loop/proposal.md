---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/parallel-execution/spec.md
  - src/parallel/queue_state.rs
  - src/parallel/orchestration.rs
  - src/parallel/types.rs
  - src/orchestration/run_control.rs
  - src/tui/state/event_handlers/completion.rs
  - src/parallel/tests/analysis_liveness_loop.rs
verifications:
  - id: failed-dependency-loop-tests
    requirement: Failed-dependency queue state converges without repeated analysis or skip events and resumes only through an accepted explicit retry edge
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output for failed-dependency scheduler-loop, retry, queue-lifetime, and event-consumer regressions
    rerun: cargo test -p cflx failed_dependency
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix Failed-Dependency Reanalysis Loop

**Change Type**: implementation

## Problem / Context

When a queued change depends on another change that has failed during the current parallel run, `filter_executable_candidates` emits `ChangeSkipped` and removes the dependent candidate from the scheduler-local queue. Reducer-owned queue intent remains authoritative, so queue reconciliation adds the same candidate back on the next scheduler pass.

Each synthetic re-addition becomes `QueueNotification`. That explicit edge bypasses unchanged-analysis-input suppression, invokes dependency analysis again, emits another identical `ChangeSkipped`, and removes the candidate again. A stable failed dependency therefore creates an unbounded cycle of analyzer invocations, repeated `Skipped ... Dependency ... failed` messages, repository scans, and log growth without a real state transition.

The fix must preserve genuine queue edges, explicit retry recovery, queue revocation, independent dispatch, and both finite and persistent scheduler lifetime behavior.

## Proposed Solution

- Preserve reducer-admitted failed-dependent candidates in the scheduler-local queue and classify them as dependency-blocked rather than repeatedly removing and restoring them.
- Emit one compatibility `ChangeSkipped` event and one authoritative `DependencyBlocked` transition when a dependent first enters a specific failed-blocker epoch. Neither event revokes accepted queue intent.
- Treat rediscovery of an already represented blocked candidate as no queue addition and not as a fresh `QueueNotification` edge. Do not suppress analysis triggered by unrelated repository/signature changes, genuine dynamic additions, bounded fail-open retry, or degraded-result retry.
- Carry an accepted `RetryError(change_id)` state change to the live scheduler as a target-ID-bearing one-shot retry edge. Consume it before reconciliation/classification, clear only that dependency's ephemeral failed marker and failed-blocker notification epoch, and arm exactly one reevaluation.
- Do not clear failure state for refused or no-op retry, ordinary `AddToQueue`, or generic queue notification. Retry intent never proves dependency success.
- Preserve normal `RemoveFromQueue`/`DequeueChange`: revoke B locally and clear its blocker notification state. A later explicit re-add is a genuine queue edge and may emit a new bounded blocker transition.
- Keep independent queued changes dispatchable. Preserve finite blocked-only termination and persistent blocked-only notification waiting.

These behaviors are one atomic scheduler-state correction. Splitting queue convergence, retry routing, and event semantics would leave either the loop or same-process recovery broken.

## Acceptance Criteria

1. Failed A with queued dependent B leaves B locally represented and dependency-blocked without apply dispatch.
2. B's first failed-blocker epoch emits exactly one `ChangeSkipped(B,A)` compatibility event and one `DependencyBlocked(B)` state transition; queue intent remains accepted.
3. Repeated unchanged wakes produce zero new reconciliation additions, analyzer invocations, skip events, blocked transitions, or identical diagnostics.
4. Genuine dynamic addition or relevant signature change still receives normal one-edge analysis behavior, and independent C reaches dispatch.
5. Only `RetryError(A)` accepted with `ReduceOutcome::Changed` sends a target-ID-bearing one-shot retry edge. Duplicate, refused, or no-op retry and generic queue wakes do not clear A's failed marker.
6. Consuming the retry edge clears A's ephemeral failed marker and prior notification epoch before classification, then performs exactly one reevaluation without treating A as resolved.
7. B remains blocked while A is queued, in-flight, unmerged, or otherwise unresolved; authoritative resolution permits B, and A's refailure establishes one new blocker epoch.
8. Dequeue removes blocked B locally and clears its notification state; explicit re-add is a genuine addition and may produce one new blocker notification.
9. With only blocked work, finite scheduling returns blocked/stalled rather than all-completed, while persistent scheduling waits for an explicit notification without polling.
10. Process restart begins with an empty ephemeral failure tracker and recomputes routing from workspace and Git evidence.
11. Real-loop paused-time regression tests complete under one second on the default test path.

## Explicit Completion Conditions

- Production `RetryError` admission carries the target ID and accepted/no-op distinction into the live scheduler.
- Failed-dependent filtering no longer creates local remove/reconcile-add churn.
- Event consumers preserve queue selection/intent when handling compatibility `ChangeSkipped`; authoritative blocked presentation comes from `DependencyBlocked`.
- Exact-count tests reject zero-analysis no-ops, log-only fixes, analyzer-only suppression, and local-queue churn.
- Tests cover accepted/no-op retry, unresolved retry, success, refailure, dequeue/re-add, genuine dynamic addition, independent dispatch, finite/persistent lifetime, and restart semantics.
- `cargo test -p cflx failed_dependency`, `cargo test -p cflx`, `cargo fmt --check`, and `cargo clippy -p cflx --all-targets -- -D warnings` pass.

## Out of Scope

- Changing dependency declaration syntax or analyzer prompts.
- Treating retry intent as dependency success.
- Weakening genuine queue-edge bypass or bounded analyzer retry contracts.
- Log-only suppression while scheduler churn remains.
- Durable failure state outside workspace/Git evidence.
