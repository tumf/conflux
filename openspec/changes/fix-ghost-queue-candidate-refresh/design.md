# Design

## Invariant

An operator-visible queued projection must correspond to one of:

1. scheduler-local loadable work,
2. a retained wake edge that will re-evaluate incomplete evidence, or
3. a typed wait/block state explaining why dispatch cannot proceed.

`candidate_not_found` must not leave none of these while preserving `queued` indefinitely.

## Boundary

The reducer owns queue intent. The scheduler owns ephemeral candidate discovery. Repository-visible `openspec/changes/<id>/proposal.md` is the authoritative active-candidate source.

A long-lived owner may observe the reducer command and repository update in either order. Candidate discovery must therefore tolerate one initially stale/missing lookup without requiring process restart.

## Minimal behavior

- On admitted dynamic hint, perform repository-root discovery.
- If the candidate is absent, do not permanently discard the hint before fresh reconciliation can classify current repository state.
- Fresh reconciliation either admits the now-visible candidate or submits the existing explicit reducer transition needed to remove unavailable queued intent.
- Do not add polling. Re-evaluation remains notification/state-transition driven and bounded.

## Safety

- Terminal, stopped, or explicitly dequeued state remains authoritative and rejects stale hints.
- Incomplete reducer evidence retains the hint fail-closed.
- A truly absent candidate cannot launch Apply.
- Marks and queue intent remain separate axes.

## Verification shape

Repository-backed tests control ordering explicitly:

1. owner/scheduler view starts before proposal creation,
2. mark/start-equivalent reducer intent and hint are admitted,
3. first lookup misses,
4. proposal appears in the repository,
5. same owner refreshes and dispatches the candidate.

A companion negative case never creates the proposal and proves the queued projection converges without Apply dispatch or warning spam.
