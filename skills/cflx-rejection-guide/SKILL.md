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
