## ADDED Requirements

### Requirement: Persistent Ready projects semantic idle

A typed persistent-scheduler idle transition MUST project external lifecycle `idle` from both direct execution-event adapters and the TUI lifecycle snapshot. Ready MUST remain `idle` when reducer-synchronized blocked, stalled, resolve-pending, or reject-pending rows remain visible but no execution is active. Repeated unchanged idle observations MUST NOT emit duplicate lifecycle transitions.

The lifecycle projection MUST return to `working` only when typed runtime evidence proves admitted workspace or base-lane work began. Queue notification, analysis, or refresh without admitted execution MUST NOT report `working`.

#### Scenario: Ready with waiting rows reports idle

- **GIVEN** a persistent scheduler is parked and the frontend execution mode is Ready
- **AND** one or more rows remain blocked, stalled, resolve-pending, or reject-pending
- **AND** no row or base-lane operation is active
- **WHEN** typed state is projected to the external lifecycle adapter
- **THEN** the adapter receives `idle`
- **AND** blocker and wait presentation remains available to the frontend

#### Scenario: admitted work ends lifecycle idle

- **GIVEN** the lifecycle adapter last received `idle` for a persistent scheduler
- **WHEN** a typed admitted-work event starts workspace preparation or scheduler-owned base-lane work
- **THEN** the adapter receives `working`
- **AND** a notification or analysis attempt without admitted work does not produce that transition

#### Scenario: unchanged idle is deduplicated

- **GIVEN** the lifecycle adapter already received `idle` for the current persistent-idle episode
- **WHEN** unchanged TUI frames or no-op wake evaluations are observed
- **THEN** no duplicate lifecycle state transition is published
