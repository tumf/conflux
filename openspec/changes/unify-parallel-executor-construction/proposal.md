---
change_type: implementation
priority: high
dependencies: []
references:
  - "src/parallel_run_service.rs"
verifications:
  - id: focused-tests
    requirement: The refactoring preserves the named behavior and proves the corrected boundary
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel_run_service.rs
    evidence: cargo test parallel_run_service --lib
    rerun: cargo test parallel_run_service --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Unify parallel executor construction

**Change Type**: implementation

## Problem / Context

`ParallelRunService::run_parallel` assembles an executor separately from `create_executor_with_queue_state`. The duplicated path omits `post_archive_action` and graceful-stop wiring, allowing headless `--push` intent to degrade to the default merge action.

## Proposed Solution

Keep CLI preflight in `run_parallel`: register dynamic changes, run `prepare_parallel_execution`, and record `ReducerCommand::AddToQueue` only after committed filtering. Then construct the executor through `create_executor_with_queue_state` and delegate to `run_parallel_order_based_with_executor`; its repeated preparation is an allowed no-op. Preserve CLI event forwarding. When all candidates are rejected without upstream integration, forwarding `ParallelStartRejected` is required but starting an empty executor and emitting `AllCompleted` is not.

## Acceptance Criteria

- `create_executor_with_queue_state` returns an executor carrying the configured `PostArchiveAction::PushToRemote` and graceful-stop source for a headless run.
- Finite CLI runs remain finite when no dynamic queue is supplied.
- Analyzer, event delivery, and queue behavior remain unchanged.

## Explicit Completion Conditions

- The scoped production or test code follows the single ownership boundary described above.
- The focused repository-local verification declared as `focused-tests` passes in one bounded invocation.
- The implementation is archived and integrated without unrelated source or contract changes.

## Change Boundary

`src/parallel_run_service.rs`, focused tests, and a test-only read accessor in `src/parallel/builder.rs` or `src/parallel/mod.rs` for observing configured `post_archive_action` and graceful-stop state. No production getter, scheduler policy, or public API change.

## Preserved Contracts

Public wire formats, unrelated lifecycle policy, and behavior outside the stated acceptance criteria remain unchanged.

## Failure Behavior

The refactoring must not replace errors with successful placeholder values or silently fall back to a different operation. Existing fail-closed behavior remains fail-closed.

## Out of Scope

Repository-wide redesign, dependency upgrades, unrelated cleanup, deployment, release, and external publication.
