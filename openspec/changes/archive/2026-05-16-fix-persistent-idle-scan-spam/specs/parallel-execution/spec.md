## MODIFIED Requirements

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle deferred merge attempts as pending/deferred outcomes rather than successful completed background merge tasks. A background merge task MUST report successful completion only when the merge attempt actually integrated the change into base or determined from repository-visible evidence that the change was already integrated.

Parallel ordinary apply dispatch MUST treat reducer terminal-error state as a stop gate. After a change emits an apply, acceptance, archive, dispatch, or workspace execution error, scheduler reanalysis, queue reconciliation, and workspace resume scans MUST NOT dispatch that same change to apply again unless explicit retry intent has cleared the recoverable error terminal state. Existing workspaces MAY remain available for operator inspection or explicit retry.

Dependency analysis MUST continue to treat an errored dependency as a dispatch blocker for dependents until the dependency is explicitly retried and reaches repository-visible success.

When configured for persistent lifetime and fully drained, the scheduler MUST remain alive without timer-driven repository/worktree polling. A fully drained persistent scheduler means there is no local queued work, no in-flight workspace task, no reducer-owned resolve/reject waiter, no active manual resolve, and no pending merge task. In that state, the scheduler SHALL wait for explicit wake events such as dynamic queue notifications or scheduler retry notifications before running queue reconciliation, worktree scans, or base-branch merge-state checks again.

The fully drained persistent idle wait MUST NOT introduce durable workflow-control state. It MUST preserve finite scheduler behavior, where finite execution exits once drained.

#### Scenario: persistent idle does not poll worktree scans

**Given**: a parallel scheduler is running with persistent lifetime
**And**: there is no queued work, no in-flight work, no resolve/reject waiter, no active manual resolve, and no pending merge task
**When**: no dynamic queue or scheduler notification is received
**Then**: the scheduler remains alive
**And**: it does not repeatedly run worktree discovery, queue reconciliation, or base-branch merge-state checks on a timer
**And**: repeated debug log bursts for idle scan commands are not emitted

#### Scenario: queue notification wakes persistent idle

**Given**: a parallel scheduler is waiting in fully drained persistent idle
**When**: a change is added through the dynamic queue
**Then**: the scheduler wakes
**And**: it checks the dynamic queue
**And**: the queued change can enter normal reanalysis and dispatch flow

#### Scenario: scheduler retry notification wakes persistent idle

**Given**: a parallel scheduler is waiting in fully drained persistent idle
**And**: reducer-owned retry work becomes eligible without adding an ordinary queued change
**When**: the scheduler is explicitly notified
**Then**: the scheduler wakes
**And**: it re-evaluates reducer-owned wait/retry state without requiring another user keypress

#### Scenario: finite scheduler still exits when drained

**Given**: a parallel scheduler is running with finite lifetime
**When**: queued work, in-flight work, resolve/reject waiters, manual resolve activity, and pending merge tasks are all drained
**Then**: the scheduler exits instead of waiting persistently
