## MODIFIED Requirements

### Requirement: Typed frontend lifecycle emission

TUI and non-interactive frontends MUST publish lifecycle state from typed runtime state and actions rather than rendered-screen scraping. The TUI lifecycle snapshot MUST represent execution mode independently from modal interaction state and MUST include only two typed row-status facts evaluated after reducer-to-TUI synchronization: whether any row is active or queued, and whether any row is blocked or stalled. A typed persistent-scheduler idle dispatch MUST project `idle` only when its guarded Running-to-Ready transition is accepted, even when blocked or stalled rows remain visible; a late idle event that leaves Select, Stopping, Error, or Stopped unchanged MUST NOT publish a new idle transition. Without an accepted persistent-idle transition, a Running blocked/stalled-only snapshot MUST continue to report `blocked`. This lifecycle publication MUST preserve the existing `EventSink` and `ReducerCommand` ownership boundaries and MUST remain observability-only.

#### Scenario: Confirmation dialog reports blocked

- **GIVEN** the TUI enters a confirmation or retry interaction requiring user input
- **WHEN** the typed TUI state changes
- **THEN** the lifecycle dispatcher receives `blocked`
- **AND** confirmation context is derived from the typed modal payload when applicable
- **AND** no terminal buffer parsing is used

#### Scenario: QR overlay preserves underlying lifecycle

- **GIVEN** the TUI displays the QR overlay while execution is idle, working, stopping, stopped, error, or waiting on blocked/stalled changes
- **WHEN** the typed TUI state is projected to an external lifecycle event
- **THEN** the lifecycle state is derived from the underlying execution and reducer-synchronized row state
- **AND** QR presentation alone does not report `blocked`

#### Scenario: Running blocked or stalled wait reports blocked

- **GIVEN** the TUI execution mode is `Running`
- **AND** at least one reducer-synchronized change row is `blocked` or `stalled`
- **AND** no change row is active or queued
- **AND** no typed persistent-scheduler idle transition has projected Ready
- **WHEN** the TUI snapshot is projected to an external lifecycle event
- **THEN** the lifecycle dispatcher receives `blocked`
- **AND** repeated unchanged frames do not emit an intervening `working` transition

#### Scenario: Persistent Ready with waiting rows reports idle

- **GIVEN** a typed persistent-scheduler idle transition changed the TUI execution mode from `Running` to Ready
- **AND** one or more reducer-synchronized rows remain blocked or stalled
- **AND** no row or base-lane operation is active
- **WHEN** the TUI snapshot is projected to an external lifecycle event
- **THEN** the lifecycle dispatcher receives `idle`
- **AND** blocker and wait presentation remains available to the frontend

#### Scenario: Active work takes precedence over waiting rows

- **GIVEN** the TUI execution mode is `Running`
- **AND** one or more rows are `blocked` or `stalled`
- **AND** at least one row has a canonical active execution status
- **WHEN** the TUI snapshot is projected to an external lifecycle event
- **THEN** the lifecycle dispatcher receives `working`

#### Scenario: Queued work preserves working lifecycle

- **GIVEN** the TUI execution mode is `Running`
- **AND** no row has an active execution status
- **AND** at least one row is queued alongside a blocked or stalled row
- **WHEN** the TUI snapshot is projected to an external lifecycle event
- **THEN** the lifecycle dispatcher receives `working`

#### Scenario: Ordinary zero-active running state remains working

- **GIVEN** the TUI execution mode is `Running`
- **AND** no row is active, queued, blocked, or stalled
- **WHEN** the TUI snapshot is projected to an external lifecycle event
- **THEN** the lifecycle dispatcher receives `working`

#### Scenario: Admitted work ends lifecycle idle

- **GIVEN** the lifecycle adapter last received `idle` for a persistent scheduler
- **WHEN** a typed admitted-work event starts workspace preparation or scheduler-owned base-lane work
- **THEN** the adapter receives `working`
- **AND** a Start notification, queue notification, or analysis attempt without admitted work does not produce that transition

#### Scenario: Unchanged persistent idle is deduplicated

- **GIVEN** the lifecycle adapter already received `idle` for the current persistent-idle episode
- **WHEN** unchanged TUI frames or no-op wake evaluations are observed
- **THEN** no duplicate lifecycle state transition is published

#### Scenario: Graceful stopping remains working

- **GIVEN** the TUI execution mode is `Stopping`
- **WHEN** the TUI snapshot is projected with any reducer-synchronized row-status combination
- **THEN** the lifecycle dispatcher receives `working`

#### Scenario: Adapter cannot mutate core state

- **GIVEN** an external lifecycle adapter is active
- **WHEN** it receives events or exits with an error
- **THEN** Core state changes still occur only through existing Core command paths
- **AND** adapter behavior cannot select the next workflow action
