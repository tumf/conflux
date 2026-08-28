---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/queue_state.rs
  - src/tui/queue.rs
  - src/web/remote_control_api
  - openspec/specs/parallel-execution/spec.md
verifications:
  - id: candidate-refresh-tests
    requirement: Accepted queue intent cannot remain queued indefinitely when dispatch candidate discovery initially misses a newly merged active OpenSpec change
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/tests/manual_resolve.rs
    evidence: cargo test parallel::tests::manual_resolve --lib
    rerun: cargo test parallel::tests::manual_resolve --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Prevent ghost queues after active proposal catalog changes

**Change Type**: implementation

## Problem / Context

A long-lived owner accepted `mark` and `start` for a proposal merged into the base after that owner started. The reducer projected `queue_intent=queued` and `execution_state=queued`, but dynamic queue ingestion returned `candidate_not_found`. The hint was consumed without dispatch, while the reducer-owned queue intent remained. Status APIs therefore reported pending work although no Apply child, phase, or further execution log existed.

The read API did not create the state. The mutation path admitted work using a view that was not guaranteed to match scheduler candidate discovery, and the missing-candidate path failed to reconcile the accepted intent with dispatch reality.

## Proposed Solution

Make queue admission and scheduler candidate discovery converge on a fresh active OpenSpec catalog view.

- Refresh or re-read the repository-visible active change catalog before final dispatch admission when an accepted queue hint initially reports `candidate_not_found`.
- Preserve an admitted hint across a transient catalog miss until the fresh reconciliation has classified it; do not silently consume the only wake edge.
- If a fresh repository-visible view proves the target is not an active change, resolve the queued projection through an explicit reducer transition and emit a typed diagnostic instead of leaving a permanent queued row.
- Keep execution marks independent. A failed queue admission must not silently revoke the operator's mark unless an existing explicit contract requires it.
- Apply the same invariant to API and TUI mutation routes because both share reducer and scheduler state.

## Acceptance Criteria

- A proposal merged into base after owner startup can be marked and started without restarting the owner.
- An initial `candidate_not_found` caused by a stale or racing catalog view is followed by a fresh repository-visible lookup and successful scheduler-local admission when the proposal exists.
- A genuinely absent proposal does not remain indefinitely as `display_status=queued` / `execution_state=queued` without scheduler-local work.
- Missing-candidate diagnostics remain bounded and identify whether the result was refreshed-and-admitted or explicitly reconciled as unavailable.
- Existing dependency, debounce, retry, terminal-state, and mark-settlement semantics remain unchanged outside this candidate-admission boundary.

## Explicit Completion Conditions

- The production path in `src/parallel/queue_state.rs` no longer consumes a candidate-miss hint while retaining an unreconciled permanent queued projection.
- Regression coverage reproduces owner-before-proposal ordering, then adds the proposal to the repository and proves admission occurs without owner restart.
- Regression coverage proves a genuinely absent candidate reaches a coherent non-dispatchable state without a ghost queued row or unbounded warning loop.
- `cargo test parallel::tests::manual_resolve --lib` passes.
- Strict and archive-gate OpenSpec validation pass.

## Out of Scope

- Changing `cflx client status` to infer process activity from logs or child processes.
- Adding filesystem polling or out-of-worktree durable workflow state.
- Changing completion notification or `cflx client wait` semantics.
- Treating `queued` alone as proof that Apply started.

## Constitutional Alignment

The change uses repository-visible workspace state and ephemeral owner state only. It introduces no durable control state outside the worktree and preserves repository-verifiable completion.
