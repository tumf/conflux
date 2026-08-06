---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/orchestration-state/spec.md
  - src/parallel/dispatch.rs
  - src/parallel/queue_state.rs
  - src/orchestration/state.rs
  - src/orchestration/operator_command.rs
  - src/web/remote_control_api/
verifications:
  - id: blocker-metadata-regressions
    requirement: "Structured external and Acceptance-owned blocker state survives lower-fidelity workspace status observations and continues to drive truthful queue and retry behavior"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust test output covering reducer event ordering, blocker projection, queue suppression, and resumable/non-resumable operator retry"
    rerun: "cargo test --lib structured_blocker_metadata_survives_workspace_blocked && cargo test --lib external_blocker_hold_survives_dispatch_status && cargo test --lib orchestration::operator_command && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Preserve external blocker metadata

**Change Type**: implementation

## Problem / Context

A validated Acceptance external blocker is first reduced through `AcceptanceGated`, which correctly creates `ExternalBlocked` state with category, origin, prerequisite owner, unblock condition, next action, and resumability. The same dispatch branch then emits a generic `WorkspaceStatusUpdated { Blocked }`. The reducer treats that lower-fidelity status as a fresh generic stall and replaces the structured state with `BlockerKind::None`, no owner or unblock condition, and `resumable: false`.

The loss is operational, not cosmetic. Queue classification no longer recognizes the Acceptance hold, so the applied workspace can be routed back through Acceptance repeatedly. The `/api/v2` action projection advertises the erased hold as `hold_not_resumable`, while the imperative retry command sees generic `stalled` state without Acceptance ownership and can accept it. Projection and command routing disagree, and neither represents the real external prerequisite truthfully.

## Proposed Solution

Make reducer precedence monotonic for blocker observations. A structured `AcceptanceGated` or `ExecutionBlocked` classification MUST remain authoritative over a later generic blocked workspace observation for the same non-terminal change. The generic observation may confirm that the workspace remains blocked, but it must not downgrade the wait kind or replace structured metadata.

Retain the generic `WorkspaceStatusUpdated { Blocked }` fallback for paths that have no structured blocker event, including legacy apply/rejection handoffs. Those paths may continue to establish conservative `stalled` metadata.

Apply the rule to both validated external blockers and Acceptance-owned execution stalls so event ordering cannot clear dispatch-suppression or retry facts. Preserve existing terminal/dequeued guards and keep all blocker state process-local.

## Atomic Scope Rationale

Reducer precedence, producer-path regression coverage, queue suppression, and retry projection describe one correctness boundary. Splitting those pieces would allow metadata to look correct in a reducer unit test while the real dispatch sequence still repeats Acceptance or exposes the wrong operator action.

## Acceptance Criteria

1. `AcceptanceGated` followed by `WorkspaceStatusUpdated { Blocked }` remains `blocked` with external blocker kind and preserves category, evidence summary, origin, prerequisite owner, unblock condition, next action, and resumability.
2. A structured `ExecutionBlocked` external prerequisite receives the same precedence regardless of whether its origin is Apply or Acceptance.
3. An Acceptance-owned non-external execution stall remains `stalled`, retains its Acceptance hold and resumability, and is not downgraded by a later generic blocked status.
4. A generic blocked workspace observation with no prior structured blocker state still establishes the existing conservative stalled fallback.
5. Structured held changes remain excluded from ordinary dispatch, so an unchanged applied workspace does not automatically repeat Acceptance.
6. Explicit retry remains available only when preserved structured evidence marks the hold resumable; non-resumable holds remain refused without losing evidence.
7. TUI, WebUI, and `/api/v2` continue to project the reducer-owned blocker kind and metadata without frontend-specific repair logic.
8. No blocker state is persisted outside the workspace or treated as Acceptance PASS, archive readiness, merge eligibility, or completion evidence.

## Explicit Completion Conditions

- `src/orchestration/state.rs` defines and enforces precedence between structured blocker events and generic blocked workspace observations.
- The real event order produced by the external-blocker path in `src/parallel/dispatch.rs` is covered by a regression test that would fail if the second event erased metadata.
- Reducer and operator-command tests prove blocker view fields, held-set membership, ordinary dispatch suppression, resumable retry, and non-resumable refusal after the full event sequence.
- A fallback regression proves unstructured `WorkspaceStatusUpdated { Blocked }` still produces the current generic stalled behavior.
- No change introduces durable runtime blocker storage or a frontend-owned lifecycle authority.
- The commands declared by `blocker-metadata-regressions` pass.

## Out of Scope

- Implementing or changing the blocked Corvus typed API.
- Automatically retrying an external prerequisite before its unblock condition changes.
- Persisting Acceptance or external blocker state across process restart.
- Redesigning external blocker validation categories or verdict parsing.
- Removing generic blocked workspace events from unrelated rejection or legacy handoff paths.
- Protecting Apply-origin non-external stalls from generic blocked-status downgrade; they carry no Acceptance/external routing ownership and remain part of any later event-contract cleanup.
