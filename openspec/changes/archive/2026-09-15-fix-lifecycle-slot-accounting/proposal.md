---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - src/parallel/dispatch.rs
  - src/parallel/queue_state.rs
  - src/parallel/conflict.rs
verifications:
  - id: lifecycle-slot-tests
    requirement: A change owns one concurrency slot continuously from workspace admission through merge settlement, including background merge and resolve
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test --lib parallel::tests::lifecycle_slot_ownership
    rerun: cargo test --lib parallel::tests::lifecycle_slot_ownership
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix lifecycle slot accounting through merge

**Change Type**: implementation

## Problem / Context

`max_concurrent_workspaces` is intended to bound concurrently admitted changes and their managed worktrees. The current scheduler releases a change's semaphore permit when its apply/acceptance/archive workspace task returns, before the independently spawned background merge settles.

`handle_workspace_completion` also removes the change from `in_flight` before `spawn_merge_task`. A queued change can therefore consume the apparent free slot while the prior change is still merging or resolving. When conflict detection later increments `auto_resolve_count`, the owner can expose three applying changes plus one resolving change under a configured limit of three.

This is not an atomic-counter ordering bug. Slot ownership ends at the wrong lifecycle boundary. `pending_merge_count` protects drain behavior but does not reserve dispatch capacity, while resolve counters reduce only future availability after resolve has already begun.

The canonical parallel-execution requirements are internally inconsistent: they require strict worktree/change limits across all phases, but separately exclude merge and `merge_wait` from `in-flight` and describe capacity recovery from background merge. This change makes lifecycle ownership authoritative and updates those requirements atomically with the implementation.

## Proposed Solution

Represent one admitted change with one owned lifecycle slot. Acquire it immediately before workspace preparation, preserve it through apply, acceptance, archive, background merge, and automatic or manual conflict resolution, and release it only when the change reaches a repository-visible settlement that ends the admitted lifecycle.

The workspace task must transfer, rather than drop, its owned permit to post-archive merge handling. The scheduler must not remove the change from capacity accounting while that transfer or background merge is pending. Resolve counters may remain observability inputs, but must not double-count a resolve already covered by a lifecycle slot.

A retained `merge_wait` remains admitted and consumes its lifecycle slot until operator resolution settles it. This intentionally applies backpressure instead of admitting increasingly divergent worktrees. A process restart must reconstruct this ownership from workspace, Git, and reducer evidence; no durable out-of-worktree slot state may be introduced.

Release the lifecycle slot on:

- successful local merge and cleanup (`merged`);
- terminal `error` or `rejected` settlement;
- explicit dequeue/not-queued settlement;
- the configured push/publication terminal settlement.

Do not release it merely because apply/archive returned, a background merge was spawned, conflict resolution started, or the change entered `merge_wait`.

## Acceptance Criteria

1. At every observable scheduler state, lifecycle-slot occupancy never exceeds `max_concurrent_workspaces`.
2. A queued change is not dispatched after another change finishes archive but remains in background merge, automatic resolve, or manual `merge_wait` resolution.
3. A change uses exactly one slot across phase transitions; merge/resolve accounting does not double-count it.
4. `merge_wait` applies backpressure and does not permit a newer worktree to replace its occupied slot before settlement.
5. Slot release on merged, terminal error/rejection, explicit dequeue, and push/publication terminal settlement wakes queued work through existing scheduler edges without polling or sticky-trigger replay.
6. Restart recovery derives occupied lifecycle changes from repository-visible workspace/Git/reducer evidence and complies with `openspec/CONSTITUTION.md`.
7. TUI/API running counts and scheduler admission use the same lifecycle membership, so a configured limit of three cannot report or execute apply 3 plus resolve 1.

## Explicit Completion Conditions

- The semaphore permit or equivalent lifecycle token is transferred from workspace execution into post-archive merge handling without an unowned interval.
- `in_flight`/capacity membership is removed only at the settlement boundaries listed above.
- Manual and automatic resolve paths share the lifecycle-slot invariant and cannot begin as an unaccounted fourth operation.
- Deterministic tests use barriers/channels rather than timing sleeps to hold archive completion, background merge, conflict resolution, and `merge_wait` states.
- `cargo test --lib parallel::tests::lifecycle_slot_ownership` passes and includes a failing-on-old-code assertion that applying plus merging/resolving occupancy never exceeds the configured maximum.
- `cflx openspec validate fix-lifecycle-slot-accounting --archive-gate` passes before archive.

## Preserved Contracts

- Base-lane merge serialization remains unchanged.
- Dependency ordering, queue intent authority, stop/dequeue semantics, acceptance, archive, push/publication behavior, and completion callbacks remain unchanged except for dispatch backpressure.
- No new dependency, external service, durable scheduler database, timer polling, or out-of-worktree workflow authority is added.

## Out of Scope

- Increasing the configured concurrency limit.
- Changing merge algorithms or conflict-resolution prompts.
- Retrofitting unrelated TUI layout or status wording.
- Cancelling already admitted work when capacity configuration is reduced at runtime.

## Fable Review Basis

Fable confirmed the observed overrun is a lifecycle ownership defect rather than a display-only or atomic-ordering problem. It identified the unowned interval between `in_flight.remove()` and conflict-time resolve accounting, and the absence of sequence-level regression coverage. This proposal adopts that diagnosis while preserving the user's stricter requirement that `merge_wait` continue to apply admission backpressure until settlement.
