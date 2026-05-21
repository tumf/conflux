## MODIFIED Requirements

### Requirement: Shared Parallel Orchestration Service

Parallel merge retry dispatch SHALL handle deferred merge attempts as pending/deferred outcomes rather than successful completed background merge tasks. A background merge task MUST report successful completion only when the merge attempt actually integrated the change into base or determined from repository-visible evidence that the change was already integrated.

Parallel ordinary apply dispatch MUST treat reducer terminal-error state as a stop gate. After a change emits an apply, acceptance, archive, dispatch, or workspace execution error, scheduler reanalysis, queue reconciliation, and workspace resume scans MUST NOT dispatch that same change to apply again unless explicit retry intent has cleared the recoverable error terminal state. Existing workspaces MAY remain available for operator inspection or explicit retry.

Dependency analysis MUST continue to treat an errored dependency as a dispatch blocker for dependents until the dependency is explicitly retried and reaches repository-visible success.

When configured for persistent lifetime and fully drained, the scheduler MUST remain alive without timer-driven repository/worktree polling. A fully drained persistent scheduler means there is no local queued work, no in-flight workspace task, no reducer-owned resolve/reject waiter, no active manual resolve, and no pending merge task. In that state, the scheduler SHALL wait for explicit wake events such as dynamic queue notifications or scheduler retry notifications before running queue reconciliation, worktree scans, or base-branch merge-state checks again.

The fully drained persistent idle wait MUST NOT introduce durable workflow-control state. It MUST preserve finite scheduler behavior, where finite execution exits once drained.

When every remaining local queued candidate is non-dispatchable without explicit external intent, the scheduler SHALL treat the run as blocked-only drained rather than continuing timer-driven reanalysis. Non-dispatchable remaining candidates include manual `MergeWait`, recoverable terminal-error changes requiring explicit retry, dependency-blocked changes, and candidates that cannot be reconstructed from repository-visible OpenSpec/workspace evidence. Blocked-only drain MUST NOT mark those changes accepted, archived, merged, or rejected; it only means the scheduler has no automatic ordinary apply work to perform.

In finite lifetime, blocked-only drain SHALL exit the scheduler loop. In persistent lifetime, blocked-only drain SHALL enter notification-driven idle wait and MUST NOT repeatedly run dependency analysis, worktree discovery, or queue reconciliation until an explicit queue/retry/merge/cancel wake event occurs.

When the parallel service receives runtime cancellation from its owner, it MUST stop scheduling new ordinary work, propagate cancellation to in-flight workspace tasks, and ensure owned agent child process groups are terminated through the command runner cleanup path before the cancelled local run is considered cleaned up.

<!-- Expected canonical result after archive: `parallel-execution` will require runtime cancellation to stop new dispatch, notify in-flight workspace tasks, and clean up owned child process groups before cancelled local run cleanup completes. -->

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

#### Scenario: finite scheduler exits with blocked-only queued work

**Given**: a parallel scheduler is running with finite lifetime
**And**: there are no in-flight workspace tasks, reducer-owned resolve/reject waiters, active manual resolves, or pending merge tasks
**And**: the only remaining queued candidates are manual `MergeWait`, terminal-error retry-required, dependency-blocked, or candidate-unavailable rows
**When**: the scheduler evaluates the next loop iteration
**Then**: the scheduler exits the running loop without invoking dependency analysis again
**And**: the remaining changes keep their reducer-visible wait/error/blocked states
**And**: no remaining change is marked accepted, archived, merged, or rejected solely because of blocked-only drain

#### Scenario: persistent scheduler idles with blocked-only queued work

**Given**: a parallel scheduler is running with persistent lifetime
**And**: there are no in-flight workspace tasks, reducer-owned resolve/reject waiters, active manual resolves, or pending merge tasks
**And**: the only remaining queued candidates are manual `MergeWait`, terminal-error retry-required, dependency-blocked, or candidate-unavailable rows
**When**: no explicit queue/retry/merge/cancel wake event is received
**Then**: the scheduler remains alive in notification-driven idle wait
**And**: it does not repeatedly invoke dependency analysis
**And**: it does not repeatedly run worktree discovery or queue reconciliation on a timer

#### Scenario: cancellation stops new parallel dispatch

**Given**: a parallel local run has received runtime cancellation from its owner
**AND**: queued ordinary work remains undispatched
**WHEN**: the scheduler evaluates the next dispatch opportunity
**THEN**: it does not dispatch new ordinary apply/archive/acceptance work
**AND**: it reports or returns a cancelled/stopped outcome rather than continuing normal scheduling

#### Scenario: cancellation reaches in-flight workspace child commands

**Given**: a parallel local run has an in-flight workspace task running an owned AI agent command
**WHEN**: runtime cancellation is received
**THEN**: cancellation is propagated to the in-flight workspace task
**AND**: the owned AI agent process group is terminated through the command runner cleanup path
**AND**: the cancelled local run is not considered cleaned up while the owned child process continues running
