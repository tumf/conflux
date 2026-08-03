# Design: Apply process-group cleanup barrier

## Scope

The barrier covers the transition from a completed Apply command to Conflux-owned repository finalization in the same managed worktree. It does not serialize unrelated worktrees and does not replace Git's own locking.

## Root Cause

The completion-grace path terminates a managed process group and waits on the group leader. Unix leader exit is not proof that all descendants have exited. A descendant may still hold stdout/stderr, mutate files, or retain Git's `index.lock` while the Apply loop starts WIP and final commit operations.

## Decisions

### Quiescence is stronger than leader exit

A successful cleanup outcome requires both leader reaping and no remaining members in the owned process group. The process manager owns this platform-specific proof. Callers must not infer it from elapsed time, PID disappearance, or lock-file age.

### Keep the existing bounded termination sequence

Use SIGTERM, wait for bounded quiescence, then SIGKILL and wait again. Return an unconfirmed outcome if members remain or membership cannot be checked within the budget. Never continue silently.

### Gate at the shared Apply boundary

The Apply loop consumes the cleanup outcome before any WIP snapshot, cleanup review, final Apply commit, rejecting handoff, or Acceptance dispatch. This keeps serial and parallel callers consistent and avoids scattered sleeps around individual Git commands.

### Test with a real descendant-held lock

The regression fixture starts a process-group leader that spawns a descendant. The descendant creates and holds the managed worktree's real `index.lock`. The test observes that Conflux finalization does not begin before group cleanup and that the lock disappears through process exit, never runtime deletion.

## Constitution Alignment

The cleanup outcome is ephemeral process-lifetime evidence. Workflow routing after restart remains derived from workspace and Git state. Acceptance is withheld unless repository-verifiable finalization can run after confirmed process cleanup, preserving truthful completion.

## Relationship to final commit lock retry

`retry-final-apply-commit-lock-contention` is independent defense for transient external Git contention after this barrier succeeds. This proposal fixes the internal lifecycle race and must not rely on retries to hide incomplete process cleanup.
