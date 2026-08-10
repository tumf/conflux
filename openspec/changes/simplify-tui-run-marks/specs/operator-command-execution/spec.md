## MODIFIED Requirements

### Requirement: Mode-aware mark and queue behavior

The service MUST treat execution marks as process-local next-run target intent. Every visible non-terminal row MUST accept mark mutation in Select, Running, Stopping, Stopped, and Error modes. Mark mutation MUST NOT change reducer queue intent, DynamicQueue, retry or resolve intent, active execution, cancellation, hooks, scheduler state, or process mode. Archived, merged, pushed, and rejected rows MUST remain outside mark mutation.

Configured Start MUST consume one coherent marked snapshot and classify current target routes independently from process mode. Marked retry-eligible recovery rows MUST use their existing typed retry routes from Ready/Select, Stopped, or process-wide Error. If retry and ordinary-start routes coexist, the command MUST dispatch only retry routes with run-wide explicit-retry semantics, report ordinary rows as deferred, and retain their marks for a later ordinary Start. If no retry route exists, ordinary startable rows MUST use the existing Start route. Running and Stopping MUST continue to refuse configured Start.

The existing all-or-nothing worktree eligibility fence and active Apply iteration-limit gate MUST run before any reducer, queue, retry-edge, scheduler, or mode effect. Other non-startable statuses MUST be reported as excluded, and zero runnable targets MUST reject without effects.

#### Scenario: Mark mutation is lifecycle-independent

**Given**: A visible non-terminal change exists in any process mode or lifecycle status
**When**: A frontend changes its execution mark
**Then**: Only the process-local mark changes
**And**: Queue, retry, resolve, execution, cancellation, hooks, scheduler, and mode remain unchanged

#### Scenario: Ready Start retries a marked change-level error

**Given**: `ProcessingError` moved `alpha` to change-level `error`
**And**: the process later projects Ready/Select
**And**: `alpha` is marked
**When**: configured Start is submitted
**Then**: `alpha` is admitted through `RetryError` with explicit-retry semantics
**And**: process-wide Error mode is not required

#### Scenario: Mixed retry and ordinary routes are separated

**Given**: marked `alpha` is retry-eligible and marked `beta` is ordinary `not queued` work
**When**: configured Start is submitted
**Then**: only `alpha` is dispatched with explicit-retry semantics
**And**: `beta` is reported as deferred and remains marked
**And**: a later configured Start may admit `beta` normally

#### Scenario: Active Apply limit rejects before effects

**Given**: a marked recovery target carries Apply iteration-limit evidence owned by a live run
**When**: configured Start is submitted
**Then**: that target is not retried
**And**: no reducer, queue, mark, retry-edge, scheduler, hook, or mode effect occurs for it

<!-- Expected canonical result after archive: `operator-command-execution` will remove mark-to-queue aliases and process-mode-only retry routing, while preserving typed retry routes, worktree eligibility, active Apply-limit safety, and start refusal during Running or Stopping. -->
