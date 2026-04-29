# Rejection / Blocked Change Reference

Use this reference only when the current repository evidence is not enough to decide between `CONFIRM`, `BLOCK`, and `RESUME`.

## State Model

Do not assume every `REJECTED.md` means terminal rejection.

A worktree-local `openspec/changes/<change-id>/REJECTED.md` can be a rejection proposal artifact that still needs review. The real disposition is one of:

- terminal `Rejected`
- resumable `Blocked`
- back to `Applying`

If the user says "rejected gated" or similar, interpret it as likely confusion between a rejection proposal and a blocked hold.

## Typical Classification

### Prefer `BLOCK` when

- the change is still valid
- the blocker is temporary or external
- retrying immediately would likely fail again
- recovery depends on dependency resolution, environment repair, archive-readiness, commit-path fixes, or spec clarification

Examples:

- missing fixture or test harness
- external dependency unavailable
- local execution / disk / capacity issue
- archive-readiness blocker
- commit-path blocker
- clarification needed before continuing

### Prefer `REJECTED` only when

- closure is more correct than recovery
- the proposal premise failed
- the change was superseded
- continuing no longer makes sense

Examples:

- requirement contradicts canonical direction
- another landed change made this proposal obsolete
- the requested outcome itself should be abandoned

### Prefer `RESUME` when

- the rejection proposal is weak or premature
- the blocker evidence is vague or speculative
- there is a plausible in-repo recovery path
- the issue is difficulty, not impossibility

## Suggested Commands

```bash
cflx openspec show <change-id>
cflx openspec validate <change-id> --strict
git status --short
```

Inspect when present:

- `openspec/changes/<change-id>/proposal.md`
- `openspec/changes/<change-id>/tasks.md`
- `openspec/changes/<change-id>/REJECTED.md`

Focus on:

- whether `REJECTED.md` exists
- whether `tasks.md` contains `Implementation Blocker` sections
- whether blocker evidence is concrete
- whether the change is still conceptually valid
- whether the blocker is temporary or closure-worthy
