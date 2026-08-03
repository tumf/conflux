# Design: Apply process-group cleanup barrier

## Scope

The barrier covers the Unix transition from a completed Apply command to Conflux-owned repository finalization in the same managed worktree. It does not serialize unrelated worktrees, replace Git locking, detect processes that escaped the owned process group, or change Windows job-object behavior.

## Confirmed Evidence and Hypothesis

Confirmed: completion-grace termination was followed by finalization before a later `index.lock` failure, and current cleanup waits for the spawned leader. Hypothesis: a descendant retained the lock after leader exit. The design removes the unsafe interval regardless of who owned the historical lock.

## Quiescence Contract

Two independent conditions are required:

1. the spawned leader has been reaped with an exit status
2. the original Unix process group is absent

The injected group probe performs signal 0 against the PGID and maps results as follows:

| Probe result | Meaning | Finalization |
| --- | --- | --- |
| success | one or more group members remain | forbidden |
| `ESRCH` | process group absent | allowed only if leader is also reaped |
| `EPERM` | group may exist but cannot be inspected | forbidden |
| any other error | presence unknown | forbidden |

A zombie or reused PGID may conservatively appear present. That can delay/fail cleanup but cannot create an unsafe success. Lock-file absence and age are never process evidence.

## Bounded State Machine

1. Send SIGTERM to the owned process group.
2. Until the graceful deadline, concurrently reap the leader and poll group presence.
3. If both conditions become true, return `Quiescent { termination: Graceful, leader_status }`.
4. Otherwise send SIGKILL to the group unless the probe already proves absence, then continue reaping/probing through a separate forceful deadline.
5. Return `Quiescent { termination: Forced, leader_status }` only when both conditions become true.
6. Otherwise return `Unconfirmed` with phase, PGID, leader-reap state, last probe result, and signal error if any.

Probe, monotonic clock, and sleeper are injected so deadline and errno branches are unit-testable without wall-clock delay.

## Caller Matrix

| Command outcome | Cleanup outcome | Apply result |
| --- | --- | --- |
| stable completion/rejecting handoff | quiescent | continue to repository handoff |
| stable completion/rejecting handoff | unconfirmed | fail Apply; no Git finalization or handoff |
| explicit cancellation | either | preserve cancellation result; record cleanup diagnostics |
| natural command completion with strict cleanup | quiescent | preserve command result |
| natural command completion with strict cleanup | unconfirmed | strict cleanup failure overrides success, but does not convert an existing command failure into success |

## Verification

Deterministic unit tests own the state-machine proof. A Unix heavy integration fixture starts a leader and descendant in one group; the descendant creates a real managed-worktree `index.lock`. An event channel records cleanup and finalization ordering. The test asserts finalization is never called while the synthetic descendant exists and that Conflux never removes the lock.

## Constitution Alignment

Cleanup evidence is ephemeral. Restart routing remains derived from workspace and Git state. Successful handoff requires repository-verifiable finalization after confirmed cleanup.

## Relationship to final commit lock retry

`retry-final-apply-commit-lock-contention` consumes this change's typed quiescence gate and therefore depends on it. Retry must not hide unconfirmed Apply cleanup.
