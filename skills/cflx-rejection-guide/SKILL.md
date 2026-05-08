---
name: cflx-rejection-guide
description: Guide users through handling Conflux/OpenSpec changes that ended in `REJECTED.md`, `Rejected`, `Blocked`, `Rejecting`, or informal states like "rejected gated". Use whenever the user asks what to do with a rejected proposal, how to recover a blocked change, how to interpret rejecting review outcomes, or how to choose between closing, blocking, and resuming a change.
---

# Conflux Rejection / Blocked Change Guide

Operator-facing guidance for deciding what to do with a change after it entered rejecting review or produced `REJECTED.md`.

## Core Decision

Normalize the situation into one of these outcomes:

- `CONFIRM`: close the change as terminal rejection
- `BLOCK`: keep the change valid but paused until prerequisites are unblocked
- `RESUME`: dismiss the rejection proposal and return to apply

If the user says "rejected gated" or similar, usually treat it as ambiguity between a rejection proposal and a blocked hold.

Read `references/guide.md` when you need fuller classification examples.

## Inspect First

Use repository evidence, not guesses.

Recommended commands:

```bash
cflx openspec show <change-id>
cflx openspec validate <change-id> --strict
git status --short
```

Read when present:

- `openspec/changes/<change-id>/proposal.md`
- `openspec/changes/<change-id>/tasks.md`
- `openspec/changes/<change-id>/REJECTED.md`

## Decision Rules

### Choose `CONFIRM` when

- `REJECTED.md` gives a concrete closure reason
- `tasks.md` blocker evidence is specific and actionable
- recovery would require changing the proposal premise itself
- another change has made this one obsolete

### Choose `BLOCK` when

- the change remains valid
- the blocker is real and concrete
- immediate retry would likely fail again
- recovery depends on information, dependency, environment, or spec follow-up

### Choose `RESUME` when

- the rejection proposal is under-evidenced
- there is a plausible in-repo recovery path
- the change should continue now rather than wait

## Recommended Next Action

- `CONFIRM`: keep the durable rejection record and treat the change as closed
- `BLOCK`: keep the change alive, make unblock/recovery work explicit in `tasks.md`
- `RESUME`: remove worktree-local `REJECTED.md`, append recovery work to `tasks.md`, return to apply

## Default Recovery Workflow for Rejected Changes

When the user wants to handle a rejected change, prefer repairing the existing change instead of creating a replacement change. A `REJECTED.md` file is a review artifact and recovery signal, not automatic proof that the change should be abandoned.

Use this sequence by default:

1. Re-review the rejected change:
   - Read `openspec/changes/<change-id>/REJECTED.md`.
   - Read `proposal.md`, `tasks.md`, `design.md` if present, and relevant spec deltas.
   - Run `cflx openspec show <change-id>` and `cflx openspec validate <change-id> --strict` when available.
2. Analyze the rejection cause:
   - Identify whether the blocker is a real terminal mismatch, missing implementation work, incomplete tests, environment failure, or agent mistake.
   - Cite concrete files, commands, or task entries rather than relying on the rejection summary alone.
3. Repair the existing change:
   - Update implementation, tests, specs, or `tasks.md` in the same `openspec/changes/<change-id>/` scope as needed.
   - Add or update recovery tasks in `tasks.md` so the reason for resuming is durable.
4. Remove `REJECTED.md` only after the repair path is understood and the change is ready to continue.
5. Verify:
   - Re-run strict OpenSpec validation.
   - Run the targeted tests or checks relevant to the repaired cause.
6. Commit the repair if the workflow or user explicitly asks for a commit; otherwise leave the repaired worktree ready for review.

Do not create a new change merely because the current change has `REJECTED.md`. Create a new change only when analysis shows the original proposal premise is obsolete, the required scope is materially different, or the user explicitly asks to abandon the original change.

## Response Format

Structure guidance as:

1. `Observed state`
2. `Interpretation`
3. `Recommended disposition`
4. `Next action`

## Output Quality Bar

- Cite concrete repository evidence when available.
- Prefer `Blocked` over `Rejected` for temporary, resumable blockers.
- Prefer `Rejected` only when closure is genuinely correct.
- If evidence is insufficient, say what must be checked next.
