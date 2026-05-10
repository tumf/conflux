---
name: cflx-rejecting
description: Review apply-generated rejection proposals (REJECTED.md) before any terminal reject decision. Returns REJECTION_REVIEW CONFIRM, RESUME, or BLOCK. CRITICAL - This skill CANNOT ask questions or request user input.
---

# Conflux Rejecting Review

Review apply-generated rejection proposals in `openspec/changes/<change-id>/REJECTED.md` before any terminal reject decision.

**CRITICAL**: This skill CANNOT ask questions to users. All decisions must be made autonomously based on available context.

## Purpose

When an apply operation determines that implementation is impossible and creates a `REJECTED.md` proposal, this skill reviews that proposal to decide whether the rejection is terminal, repository-fixable, or a real non-terminal stalled blocker.

## Required Checks

- If `openspec/CONSTITUTION.md` exists, read it before rejection review and treat it as higher-priority project law than proposal/spec deltas.

1. Confirm `openspec/changes/<change-id>/REJECTED.md` exists and contains a concrete reason.
2. Confirm blocker evidence in `tasks.md` (`## Implementation Blocker #N`) is specific and actionable.
3. Decide one outcome only:
   - `CONFIRM`: terminal rejection evidence proves the change intent is invalid, obsolete, contradictory, or constitution-violating.
   - `RESUME`: reject proposal is dismissed and change must return to apply because the issue is repository-fixable or the evidence is insufficient.
   - `BLOCK`: reject proposal describes a real non-terminal blocker such as infrastructure outage, missing non-mockable credential, rate limit, port conflict, package/Docker registry timeout, or pending managed verification job.
4. Output exactly one final marker line:
   - `REJECTION_REVIEW: CONFIRM`
   - `REJECTION_REVIEW: RESUME`
   - `REJECTION_REVIEW: BLOCK`

## Decision Criteria

### CONFIRM when:
- The blocker evidence proves the proposal/change intent itself is invalid, obsolete, contradictory, or violates `openspec/CONSTITUTION.md`
- The root cause is a spec contradiction or required spec amendment, not merely an unavailable verification dependency
- Terminal closure is the correct lifecycle and base branch should record `REJECTED.md`

### RESUME when:
- The blocker evidence is vague, speculative, or lacks concrete file/line references
- The blocker could be resolved by a different implementation approach within the repo
- The apply agent did not exhaust reasonable alternatives before escalating
- The rejection reason is about difficulty rather than impossibility

### BLOCK when:
- The change intent remains valid, but required verification or execution cannot currently complete for an external/local infrastructure reason
- Examples: Docker daemon unavailable, Docker image pull DNS timeout, package registry timeout, external service outage, missing non-mockable external credential, rate limit, port conflict, or managed verification job still running/pending
- The correct lifecycle is resumable stalled hold, not terminal rejected

## Outcome Rules

- On `REJECTION_REVIEW: CONFIRM`, runtime finalizes rejection flow and base branch records only `openspec/changes/<change-id>/REJECTED.md`.
- On `REJECTION_REVIEW: RESUME`, runtime removes worktree-local `REJECTED.md`, appends at least one unchecked non-rejection recovery task to `tasks.md`, and routes directly back to apply.
- On `REJECTION_REVIEW: BLOCK`, runtime removes worktree-local `REJECTED.md`, appends/retains recovery work in `tasks.md`, records a non-terminal stalled hold, and MUST NOT write base-branch `REJECTED.md`.
- Rejecting review MUST NOT output `ACCEPTANCE: GATED`; that marker is reserved for acceptance operation output.

## Output Contract

Output exactly ONE verdict marker at the end on its own line:

```
REJECTION_REVIEW: CONFIRM
```
or
```
REJECTION_REVIEW: RESUME
```
or
```
REJECTION_REVIEW: BLOCK
```

**Never**:
- Ask user for clarification
- Stop and wait for input
- Output both markers
- Output acceptance-related markers
