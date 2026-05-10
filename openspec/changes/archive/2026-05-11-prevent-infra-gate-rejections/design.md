# Design: Non-terminal Infrastructure Verification Blockers

## Existing Behavior

Conflux already has separate concepts for:

- `WaitState::DependencyBlocked`: queue-side dependency blocking, displayed as `blocked`.
- `WaitState::Stalled`: execution-side resumable holds, displayed as `stalled`.
- `TerminalState::Rejected`: terminal rejection, surfaced by base-branch `REJECTED.md` and read-only TUI/dashboard rows.

Apply can already emit `APPLY_BLOCKED/marker.md` and route to a stalled workspace without rejection flow. However, the parallel acceptance path still routes `AcceptanceResult::Gated` into `execute_rejection_flow()`, which writes terminal `REJECTED.md` on the base branch.

## Desired Classification

The core classification is:

```text
Valid change + infrastructure unavailable = stalled, resumable
Invalid change premise = rejected, terminal
Repository-fixable behavior failure = fail/continue to apply
Dependency unavailable due queued/rejected/missing dependency = blocked dependency state
```

Infrastructure and pending-verification examples include Docker pull DNS timeout, Docker daemon unavailable, package registry timeout, missing non-mockable external credentials, third-party outage, rate limits, port conflicts, and managed verification jobs without terminal evidence.

## Routing Changes

### Acceptance stalled-hold routing

Acceptance `gated` remains a compatibility protocol token, but runtime handling should treat it as a stalled hold rather than terminal rejection.

The routing should emit or apply an event that produces:

- activity: idle
- wait state: `Stalled`
- terminal state: none
- blocker metadata: structured category, gate/phase, observed error, resumability, and next action
- preserved worktree/WIP context where available

The routing must not call `execute_rejection_flow()` unless a separate terminal rejection review or classifier proves the change itself must be closed.

### Rejecting review tri-state

Rejecting review has three outcomes:

- `CONFIRM`: terminal rejection; write base `REJECTED.md` and clean up rejected worktree as today.
- `RESUME`: rejection proposal is invalid or repo-fixable; clear worktree-local `REJECTED.md` and return to apply.
- `BLOCK`: rejection proposal describes a real non-terminal blocker; clear worktree-local `REJECTED.md`, append/retain recovery details, and transition to stalled hold.

Code already has a `RejectionReviewVerdict::Block` shape; the change should make the shipped skill contract and tests match it.

## Blocker Metadata

The existing `BlockedMetadata` can be used or extended. It should support enough information for downstream operators without relying on non-workspace durable state for routing.

Minimum semantic fields to preserve in some structured form:

- blocker category (`infrastructure`, `credential`, `external_service`, `pending_verification`, or equivalent)
- failed gate or phase
- observed error summary
- resumable flag or equivalent unambiguous wording
- recommended next action
- worktree preservation note

## Prompt / Skill Boundary

The distributed skills are part of the product surface because they are embedded into the binary and installed for external agents. They must not drift from runtime contracts.

Required guidance updates:

- `cflx-rejecting` documents `REJECTION_REVIEW: BLOCK` as valid.
- `cflx-accept` states that `gated` is a stalled-hold compatibility token and must not imply terminal rejection.
- `cflx-apply` avoids instructing agents to create `REJECTED.md` for recoverable infrastructure blockers.

## Test Strategy

Use small unit tests for routing/classification and embedded skill drift because heavy Docker/network tests are unnecessary and would violate the repository rule that default tests should remain fast.

Representative error strings are sufficient for regression classification tests; no test should require real Docker, network access, package registries, or external credentials.
