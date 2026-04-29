---
name: cflx-rejecting
description: Review apply-generated rejection proposals (REJECTED.md) before any terminal reject decision. Returns REJECTION_REVIEW CONFIRM or RESUME. CRITICAL - This skill CANNOT ask questions or request user input.
---

# Conflux Rejecting Review

Review apply-generated rejection proposals in `openspec/changes/<change-id>/REJECTED.md` before any terminal reject decision.

**CRITICAL**: This skill CANNOT ask questions to users. All decisions must be made autonomously based on available context.

## Purpose

When an apply operation determines that implementation is impossible and creates a `REJECTED.md` proposal, this skill reviews that proposal to decide whether the rejection is valid or should be dismissed so the change can return to apply.

## Required Checks

1. Confirm `openspec/changes/<change-id>/REJECTED.md` exists and contains a concrete reason.
2. Confirm blocker evidence in `tasks.md` (`## Implementation Blocker #N`) is specific and actionable.
3. Decide one outcome only:
   - `CONFIRM`: reject proposal is valid and should be finalized.
   - `RESUME`: reject proposal is dismissed and change must return to apply.
4. Output exactly one final marker line:
   - `REJECTION_REVIEW: CONFIRM`
   - `REJECTION_REVIEW: RESUME`

## Decision Criteria

### CONFIRM when:
- The blocker evidence is concrete and references specific files, lines, or command output
- The root cause is genuinely outside the scope of what the apply agent can resolve (spec contradiction, truly non-mockable external dependency)
- Unblock actions require human judgment, external system changes, or spec amendments

### RESUME when:
- The blocker evidence is vague, speculative, or lacks concrete file/line references
- The blocker could be resolved by a different implementation approach within the repo
- The apply agent did not exhaust reasonable alternatives before escalating
- The rejection reason is about difficulty rather than impossibility

## Outcome Rules

- On `REJECTION_REVIEW: CONFIRM`, runtime finalizes rejection flow and base branch records only `openspec/changes/<change-id>/REJECTED.md`.
- On `REJECTION_REVIEW: RESUME`, runtime removes worktree-local `REJECTED.md`, appends at least one unchecked non-rejection recovery task to `tasks.md`, and routes directly back to apply.
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

**Never**:
- Ask user for clarification
- Stop and wait for input
- Output both markers
- Output acceptance-related markers
