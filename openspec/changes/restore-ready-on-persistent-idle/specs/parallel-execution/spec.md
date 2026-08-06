## ADDED Requirements

### Requirement: Persistent scheduler idle is a typed resumable transition

When a persistent scheduler reaches a coherent fully-drained or stable blocked/waiting-only state, it MUST remain alive in its existing event-driven wait and MUST emit one typed persistent-idle transition before parking. When that transition observes frontend Running, it MUST project Ready/`select` and set a process-local `persistent_scheduler_idle` idle-episode fact without claiming completion or changing reducer-owned lifecycle, queue intent, blockers, wait state, worktree evidence, diagnostics, or process-local execution marks. A late transition observed in Select, Stopping, Error, or Stopped MUST leave both mode and the idle-episode fact unchanged.

The transition MUST be edge-triggered once per continuous idle episode. Repeated scheduler evaluation or a wake notification that starts no admitted work MUST NOT emit another idle transition. Start notification and an idle-origin graceful-stop request MUST retain the idle-episode fact. The edge MAY rearm only after typed evidence shows that admitted work has begun. Queue notification, dependency analysis, and catalog refresh alone MUST NOT project Running.

The first typed admitted-work event after idle MUST restore Running and clear the idle-episode fact for ordinary workspace preparation and scheduler-owned base-lane work. Error or Stopped MUST also clear the fact. Persistent lifetime, explicit wake sources, and non-polling behavior MUST remain unchanged.

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
- **WHEN** the idle predicate is evaluated again or a notification wakes the loop without admitting work
- **THEN** no additional persistent-idle transition is emitted
- **AND** no scheduler or lifecycle consumer observes a second idle edge

#### Scenario: admitted work restores Running

- **GIVEN** TUI and Web are Ready while the persistent scheduler remains alive
- **WHEN** an existing typed event proves workspace preparation, resolve, rejection review, or another base-lane operation has actually begun
- **THEN** TUI and Web project Running
- **AND** `persistent_scheduler_idle` becomes false in the same projection
- **AND** a preceding queue notification or analysis attempt by itself does not project Running

#### Scenario: terminal frontend modes are retained

- **GIVEN** a late persistent-idle event is delivered while a frontend is Error, Stopping, or Stopped
- **WHEN** the frontend handles the event
- **THEN** that terminal or transitional mode is unchanged
- **AND** no success completion message is added
