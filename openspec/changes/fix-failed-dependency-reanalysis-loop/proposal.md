---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/parallel-execution/spec.md
  - src/parallel/queue_state.rs
  - src/parallel/orchestration.rs
  - src/parallel/types.rs
  - src/parallel/tests/analysis_liveness_loop.rs
verifications:
  - id: failed-dependency-loop-tests
    requirement: Failed-dependency queue state converges without repeated analysis or skip events and resumes after explicit retry
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output for the failed-dependency scheduler-loop regression tests
    rerun: cargo test -p cflx failed_dependency
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix Failed-Dependency Reanalysis Loop

**Change Type**: implementation

## Problem / Context

When a queued change depends on another change that has failed during the current parallel run, `filter_executable_candidates` emits `ChangeSkipped` and removes the dependent candidate from the scheduler-local queue. Reducer-owned queue intent remains authoritative, so queue reconciliation adds the same candidate back on the next scheduler pass.

Each synthetic re-addition is classified as `QueueNotification`. That explicit edge bypasses unchanged-analysis-input suppression, invokes dependency analysis again, emits another identical `ChangeSkipped`, and removes the candidate again. A stable failed dependency can therefore create an unbounded cycle of analyzer invocations, repeated operator-visible `Skipped ... Dependency ... failed` messages, repository scans, and log growth without any state transition.

The canonical `Dependent Change Skipping` requirement already requires failed dependents not to dispatch, while unresolved dependency work remains queued. The implementation must represent that stable blocked state without destructive queue churn.

## Proposed Solution

- Preserve reducer-admitted failed-dependent candidates in the scheduler-local queue and classify them as dependency-blocked rather than removing and re-adding them.
- Emit the failed-dependency skip/block transition and its operator-visible diagnostic once for an unchanged blocker fingerprint.
- Treat reconciliation of an already-known blocked candidate as no queue addition and therefore not as a fresh `QueueNotification` edge.
- Suppress analyzer invocation when all remaining candidates are blocked or waiting and no new state transition makes work dispatchable.
- Reconcile `FailedChangeTracker` with accepted explicit retry and successful completion transitions so a dependency is not permanently failed within the same process. Retry admission may clear dispatch suppression, but repository evidence and normal dependency checks still determine whether dependents become executable.
- Keep independent queued changes dispatchable while failed-dependent candidates remain blocked.

These behaviors form one atomic scheduler-state correction: queue preservation without retry-state reconciliation would make dependents permanently blocked, while retry reconciliation without queue preservation would leave the re-add/remove loop intact.

## Acceptance Criteria

1. If change B has accepted queue intent and depends on failed change A, B remains represented in the scheduler-local queue but is not dispatched.
2. Repeated timer wakes and queue reconciliation over unchanged A/B state do not invoke the analyzer again and do not emit repeated `ChangeSkipped` or identical failed-dependency diagnostics.
3. Reconciliation of an already-known failed-dependent B reports no new queued addition and does not create a `QueueNotification` analysis edge.
4. An unrelated queued change C remains eligible for normal analysis and dispatch while B is dependency-blocked.
5. An accepted explicit retry for A clears the stale in-process failed classification for A and causes one fresh scheduler evaluation.
6. B remains blocked until normal repository and dependency evidence shows A resolved; retry intent alone does not prove A successful or permit B to bypass dependency checks.
7. If retried A fails again, the failed classification and bounded blocker notification are established again without starting an unbounded loop.
8. Regression coverage drives the real scheduler loop with paused time and completes in under one second on the default test path.

## Explicit Completion Conditions

- The failed-dependency filtering path no longer removes reducer-admitted blocked candidates in a way that reconciliation immediately reverses.
- Queue reconciliation distinguishes a real newly admitted candidate from an already represented blocked candidate.
- `FailedChangeTracker` exposes and uses a narrowly scoped failure-clear transition tied to accepted explicit retry or authoritative success handling.
- Loop-level tests demonstrate bounded analyzer invocation and bounded skip diagnostics across multiple scheduler wakes.
- Tests demonstrate blocked-to-retry-to-resolved and blocked-to-retry-to-failed transitions without weakening independent dispatch.
- `cargo test -p cflx failed_dependency`, `cargo fmt --check`, and `cargo clippy -p cflx --all-targets -- -D warnings` pass.

## Out of Scope

- Changing dependency declaration syntax or dependency-analysis prompts.
- Treating retry intent as successful dependency resolution.
- Removing explicit queue-edge bypass behavior for genuine new queue additions.
- Hiding repeated work through log-only deduplication while scheduler churn remains.
- Changing terminal-error policy for the failed change itself.
