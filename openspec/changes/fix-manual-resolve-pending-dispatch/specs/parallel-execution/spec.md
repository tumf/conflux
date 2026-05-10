## MODIFIED Requirements

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle stale or deleted retry worktree paths without running merge-readiness `git status` commands in missing directories. Archive-completion verification used for base merge readiness MUST use an existing repository root or fail before command execution with a bounded stale-retry outcome.

When a deferred merge retry no longer has a valid worktree path, the scheduler MUST derive the next action from repository-visible evidence: already merged changes are treated as completed, valid archived changes may be retried from an existing root, and changes with no valid retry evidence have retry intent cleared or suppressed with a single diagnostic.

When a user registers manual retry intent while other apply/archive work is in flight, the scheduler MUST preserve reducer-owned `ResolveWait`, continue unrelated work, and retry the pending merge after the in-flight work releases scheduler/base-lane capacity. If queue notification or slot release occurs and reducer-owned `ResolveWait` still exists, the scheduler MUST reevaluate and dispatch that retry instead of leaving it stranded indefinitely.

#### Scenario: manual resolve pending dispatches after capacity becomes available

**Given**: change `alpha` is recorded in reducer-owned `ResolveWait`
**And**: unrelated change `beta` is still consuming scheduler/base-lane capacity
**And**: the TUI row for `alpha` is visible as `resolve pending`
**When**: `beta` completes or another loop trigger releases capacity and the scheduler reevaluates pending retry work
**Then**: the scheduler dispatches retry work for `alpha`
**And**: operator-visible logs show retry dispatch starting for `alpha`
**And**: `alpha` does not remain indefinitely in `resolve pending` solely because `beta` was previously active

#### Scenario: queue notification without reducer-owned resolve wait does not create false retry work

**Given**: the scheduler receives a queue notification
**And**: no reducer-owned `ResolveWait` exists for change `alpha`
**When**: the scheduler reevaluates retry work
**Then**: no retry dispatch starts for `alpha`
**And**: the system does not treat a display-only pending state as scheduler-owned retry work

#### Scenario: repeated unchanged manual resolve blocker is bounded

**Given**: change `alpha` remains in reducer-owned manual resolve pending
**And**: repeated scheduler reevaluations observe the same unchanged repository-visible blocker signature for `alpha`
**When**: retry reevaluation runs repeatedly without any relevant state change
**Then**: identical user-visible retry/blocker diagnostics are deduped, rate-limited, or summarized
**And**: the scheduler continues to preserve truthful pending/blocker state
**And**: a changed blocker signature emits a fresh diagnostic
