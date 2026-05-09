# Design: Fix Terminal Dependency Blocked Retry

## Current Behavior

Conflux execution discovery skips active change directories that contain `REJECTED.md`, but dependency status rendering and dependency target classification do not consistently expose that terminal state. In a dependent run, this can appear as a pending dependency in CLI output while the scheduler later treats the target as unavailable and repeatedly reports missing/blocking diagnostics.

The scheduler currently keeps unresolved dependent changes queued. That is correct for recoverable blockers, but unchanged terminal blockers should not cause repeated operator-visible errors every loop.

## Target Model

Dependency target classification becomes:

- `Queued`: active proposal exists and is runnable candidate evidence.
- `InFlight`: dependency id is reported as currently executing.
- `Archived`: archive evidence exists and the dependency is satisfied.
- `Rejected`: proposal directory has terminal `REJECTED.md` evidence.
- `Missing`: no queued, in-flight, archived, or rejected evidence exists.

`Rejected` and `Missing` are both fail-closed dispatch blockers, but they are diagnostically distinct.

## Repository-State Evidence

The classifier must derive decisions from workspace/base git state only:

- active proposal paths under `openspec/changes/<id>/proposal.md`
- rejected marker paths under `openspec/changes/<id>/REJECTED.md`
- in-flight markers already supplied to the scheduler
- archive paths under `openspec/changes/archive/`
- existing base-branch tree checks where the scheduler already uses them

No external log, cache, metrics file, or TUI-only memory may become authoritative workflow-control input.

## Diagnostic Deduplication

The scheduler should compute a blocker signature for each blocked change, for example:

```text
(change_id, [(dependency_id, dependency_class)])
```

When a change remains blocked with the same signature, Conflux should preserve the blocked runtime state but suppress repeated operator-visible warning/error events. Debug-level traces may remain available if needed, but the TUI log and standard run log should not grow with identical user-facing diagnostics on every loop.

When the signature changes, the scheduler must emit a new diagnostic and update the recorded signature. Signature changes include:

- dependency class changes from rejected/missing/queued/in-flight to archived
- dependency class changes from missing to rejected, queued, or in-flight
- dependency id set changes
- change removal/re-addition that creates a new queued intent

## Dispatch Semantics

- `Archived` dependencies do not block dispatch.
- `Queued` and `InFlight` dependencies block until the existing base-merge/archive resolution check says they are resolved.
- `Rejected` dependencies block dispatch and do not automatically resolve.
- `Missing` dependencies block dispatch and remain fail-closed.

Existing dependency-resolved behavior, including forced fresh workspace recreation after a recoverable blocker resolves, must remain intact.

## CLI Semantics

`cflx openspec list` and `show` should use the same target evidence vocabulary as scheduler classification. A rejected dependency must be visible as `rejected` so operators can distinguish it from a pending dependency that can eventually complete.

This proposal does not require changing whether rejected changes are listed as their own rows. The required invariant is that dependency status must not claim a rejected dependency is pending or missing when repository-visible rejected evidence exists.
