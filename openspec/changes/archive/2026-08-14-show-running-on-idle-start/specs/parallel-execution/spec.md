## MODIFIED Requirements

### Requirement: Persistent scheduler idle is a typed resumable transition

When a persistent scheduler reaches a coherent fully-drained or stable blocked/waiting-only state, it MUST remain alive in its existing event-driven wait and MUST emit one typed persistent-idle transition before parking. When that transition observes frontend Running, it MUST project Ready/`select` and set a process-local `persistent_scheduler_idle` idle-episode fact without claiming completion or changing reducer-owned lifecycle, queue intent, blockers, wait state, worktree evidence, diagnostics, or process-local execution marks. A late transition observed in Select, Stopping, Error, or Stopped MUST leave both mode and the idle-episode fact unchanged.

The transition MUST be edge-triggered once per continuous idle episode. Repeated scheduler evaluation or a generic wake notification that adds no scheduler-visible intent MUST NOT emit another idle transition. An accepted Start outcome MAY close the frontend idle episode and project Running before admitted workspace work begins only after reducer queue or explicit-retry intent commits. The scheduler MUST rearm the idle edge when a coherent reconciliation pass observes at least one queued row or an unconsumed accepted explicit-retry hold. Dependency analysis, catalog refresh, and generic notification alone MUST NOT rearm the edge or project Running.

The first typed admitted-work event after idle MUST restore Running and clear the idle-episode fact when no accepted Start has already done so, including non-Start queue admission, and MUST continue to establish actual workspace/base-lane execution. Error or Stopped MUST also clear the fact. Application mode and actual execution observation remain separate: accepted Start MAY project Running, while active-work and phase facts MUST still require typed dependency-analysis or lifecycle evidence. If reconciled intent produces no admitted work, the next persistent park MUST emit one fresh idle transition and return the frontends to Ready. Persistent lifetime, explicit wake sources, and non-polling behavior MUST remain unchanged.

#### Scenario: fully drained persistent scheduler projects Ready and stays alive

- **GIVEN** coherent reducer evidence proves no queued, in-flight, resolve/reject-wait, manual-resolve, merge, or push work remains
- **AND** scheduler lifetime is persistent
- **WHEN** the scheduler enters its event-driven idle wait
- **THEN** it emits one typed persistent-idle transition
- **AND** TUI and Web project Ready / `app_mode: select` with `persistent_scheduler_idle: true`
- **AND** the same scheduler remains alive for explicit wake
- **AND** no completion success is emitted

#### Scenario: blocked or waiting-only idle preserves row evidence

- **GIVEN** no agent or base-lane operation is executing
- **AND** coherent reducer state contains only blocked, stalled, resolve-wait, or reject-wait work
- **WHEN** the persistent scheduler parks
- **THEN** frontend execution mode becomes Ready
- **AND** each row retains its reducer-derived status, blocker metadata, queue intent, worktree evidence, diagnostics, and execution mark

#### Scenario: duplicate idle evaluation is suppressed

- **GIVEN** a persistent scheduler already emitted the idle transition
- **WHEN** the idle predicate is evaluated again or a notification wakes the loop without queued rows or an unconsumed explicit-retry hold
- **THEN** no additional persistent-idle transition is emitted
- **AND** no scheduler or lifecycle consumer observes a second idle edge

#### Scenario: admitted work restores Running

- **GIVEN** TUI and Web are Ready while the persistent scheduler remains alive
- **AND** no accepted Start outcome has already projected Running
- **WHEN** an existing typed event proves workspace preparation, resolve, rejection review, or another base-lane operation has actually begun
- **THEN** TUI and Web project Running
- **AND** `persistent_scheduler_idle` becomes false in the same projection
- **AND** a preceding non-Start queue notification or analysis attempt by itself does not project Running

#### Scenario: terminal frontend modes are retained

- **GIVEN** a late persistent-idle event is delivered while a frontend is Error, Stopping, or Stopped
- **WHEN** the frontend handles the event
- **THEN** that terminal or transitional mode is unchanged
- **AND** no success completion message is added

#### Scenario: Accepted idle Start rearms the next idle edge

- **GIVEN** TUI and Web are Ready while a persistent scheduler remains alive
- **AND** shared run control accepts Start for one or more marked targets
- **WHEN** reducer queue or explicit-retry intent commits and the scheduler observes a queued row or unconsumed retry hold
- **THEN** the accepted outcome projects Running immediately
- **AND** the persistent-idle latch is rearmed exactly once for the new run episode
- **AND** the existing scheduler is notified without spawning another scheduler

#### Scenario: No admitted work parks back to Ready

- **GIVEN** accepted Start projected Running and rearmed the idle edge
- **WHEN** analysis or classification starts no workspace or base-lane operation
- **AND** the coherent persistent-idle predicate becomes true again
- **THEN** the scheduler emits one fresh persistent-idle transition before parking
- **AND** TUI and Web return to Ready
- **AND** the same scheduler remains alive for later explicit wake

#### Scenario: Non-Start queue admission preserves typed resume

- **GIVEN** TUI and Web are Ready while the persistent scheduler remains alive
- **WHEN** a client queue delta adds a queued row without an accepted Start outcome
- **THEN** the queue addition rearms the scheduler idle edge but does not itself project Running
- **AND** typed admitted-work evidence projects Running if work starts
- **AND** a no-work idle event delivered while Select is guarded as a no-op by each frontend projection

#### Scenario: Actual work evidence remains typed

- **GIVEN** accepted Start has projected Running before dependency analysis begins
- **WHEN** dependency analysis or workspace/base-lane work actually starts
- **THEN** its existing typed event opens the corresponding process activity or lifecycle phase
- **AND** application mode alone is not used as proof of that activity

<!-- Expected canonical result after archive: persistent idle remains edge-triggered and non-polling while accepted Start opens Running immediately, level-based queue/retry observation rearms no-work closure, and non-Start queue admission retains typed resume behavior. -->
