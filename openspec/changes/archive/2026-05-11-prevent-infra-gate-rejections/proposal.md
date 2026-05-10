---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/agent-prompts/spec.md
  - src/parallel/dispatch.rs
  - src/orchestration/state.rs
  - src/orchestration/rejection.rs
  - src/execution/apply.rs
  - skills/cflx-accept/SKILL.md
  - skills/cflx-rejecting/SKILL.md
---

# Prevent Infrastructure Verification Blockers from Becoming Terminal Rejections

**Change Type**: implementation

## Problem / Context

Conflux can currently turn a required verification gate that cannot complete for external or local infrastructure reasons into a terminal rejection by writing `openspec/changes/<change_id>/REJECTED.md` on the base branch.

A concrete example is a valid and active change whose Docker API startup smoke gate could not run because Docker images could not be pulled due DNS/network timeouts and were not present locally. That condition does not prove the proposal, spec, or implementation intent is invalid. It is a resumable environmental blocker.

This conflicts with the intended lifecycle distinction:

- Valid change plus infrastructure unavailable is a non-terminal stalled hold.
- Invalid or obsolete change premise is terminal rejection.
- Repository-fixable implementation or acceptance issues return to apply/fix.

The existing codebase already distinguishes dependency `blocked` from execution `stalled`. This change should preserve that distinction and use `stalled` as the operator-facing lifecycle for resumable execution holds, while keeping `blocked` for dependency queue waiting.

## Proposed Solution

Update acceptance, rejecting, and prompt guidance so infrastructure and pending-verification blockers are routed to non-terminal stalled holds instead of terminal rejection flow.

The implementation should:

1. Stop routing every acceptance stalled-hold compatibility verdict (`AcceptanceResult::Gated`) through `execute_rejection_flow()`.
2. Record infrastructure and pending-verification holds as reducer-owned stalled state with structured blocker metadata and without base-branch `REJECTED.md`.
3. Keep terminal `REJECTED.md` creation limited to explicit terminal rejection evidence, such as rejecting review `CONFIRM` for invalid, obsolete, contradictory, or constitution-violating change intent.
4. Formalize `REJECTION_REVIEW: BLOCK` in the shipped rejecting skill and mirror guidance so apply-generated rejection proposals can be downgraded to a resumable stalled hold.
5. Ensure agent-exec or managed verification jobs that are still running/pending do not become pass or terminal rejection evidence.

The implementation MUST keep workflow-control inputs workspace-local in accordance with the constitution. Logs and UI state may describe the blocker, but next-action routing must be derivable from workspace file/git state plus emitted runtime events produced from workspace-visible execution evidence.

## Acceptance Criteria

- Docker pull DNS/network timeout during a required smoke gate does not create base-branch `openspec/changes/<change_id>/REJECTED.md`.
- Docker daemon unavailable, image unavailable, package registry timeout, external service outage, missing non-mockable external credentials, rate limit, port conflict, and still-running managed verification jobs are treated as non-terminal stalled holds unless independent evidence proves the change intent is invalid.
- Acceptance stalled-hold compatibility verdicts become `WaitState::Stalled` / display `stalled` and preserve resumable worktree context rather than invoking terminal rejection flow.
- `REJECTED.md` is created only for terminal rejection evidence.
- A rejecting review `BLOCK` verdict removes/clears the worktree-local rejection proposal artifact, keeps the change resumable, records blocker metadata, and does not write base-branch `REJECTED.md`.
- Downstream state consumers can distinguish terminal `rejected`, non-terminal `stalled`, dependency `blocked`, and repository-fixable acceptance/apply failures.
- Distributed skills and embedded drift tests document `REJECTION_REVIEW: BLOCK` as a valid output and document `gated` only as a compatibility protocol token for stalled holds, not as terminal rejection.

## Explicit Completion Conditions

The change is complete when repository evidence shows:

- `src/parallel/dispatch.rs` or equivalent acceptance routing no longer calls `execute_rejection_flow()` merely because acceptance returned `AcceptanceResult::Gated`.
- The reducer receives or derives a non-terminal stalled hold for acceptance infrastructure blockers, with structured metadata containing at least failed phase/gate, error summary, resumability, and next action.
- Existing terminal rejecting review `CONFIRM` behavior still writes base-branch `REJECTED.md` and dequeues the change.
- Rejecting review `BLOCK` is documented in `skills/cflx-rejecting/SKILL.md`, any mirrored references, and embedded skill contract tests.
- Regression tests prove both the non-terminal infrastructure-blocker path and terminal rejection path.
- `cflx openspec validate prevent-infra-gate-rejections --strict` passes before implementation and archive validation remains available after implementation.

## Out of Scope

- Introducing a separate durable out-of-worktree workflow database.
- Renaming the existing dependency `blocked` lifecycle.
- Replacing the compatibility acceptance token `{"acceptance":"gated"}` with a new parser-level `stalled` verdict in this change.
- Implementing Docker/network recovery itself.
- Changing OpenSpec validation semantics unrelated to rejection/stalled classification.
