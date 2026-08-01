## MODIFIED Requirements

### Requirement: Orchestration loop runs apply and archive

The orchestration loop SHALL run the configured Apply and Acceptance commands and parse their machine-readable outcomes. It SHALL continue to accept `gated` and legacy `blocked` acceptance tokens as compatibility input, but token spelling alone SHALL NOT determine operator-facing lifecycle status.

Conflux SHALL classify validated non-repository prerequisite evidence as `blocked`. It SHALL classify no-progress, repeated-finding, and exhausted-retry execution stops as `stalled`. Invalid bare compatibility blocker input SHALL use bounded protocol correction and SHALL set neither lifecycle state without sufficient evidence.

#### Scenario: Structured external prerequisite displays blocked

- **GIVEN** Apply or Acceptance emits complete structured evidence for a non-repository prerequisite
- **WHEN** the orchestration loop validates and classifies the result
- **THEN** the change lifecycle status is `blocked`
- **AND** the status detail identifies the blocker kind as external

#### Scenario: No-progress execution displays stalled

- **GIVEN** a change makes no semantic progress, repeats a finding, or exhausts its retry policy
- **WHEN** the orchestration loop finalizes the execution hold
- **THEN** the lifecycle status is `stalled`
- **AND** it is not presented as a dependency or external wait

#### Scenario: Legacy token remains input compatibility only

- **GIVEN** Acceptance emits legacy `blocked` or `gated`
- **WHEN** the orchestration loop parses the verdict
- **THEN** the token remains accepted as compatibility syntax
- **AND** complete structured facts are still required for external `blocked`
- **AND** bare input follows bounded protocol correction

### Requirement: Processing Item Spinner Animation

The TUI SHALL display phase-specific vocabulary for active work: `applying`, `accepting`, `archiving`, and `resolving`. It SHALL display `blocked` for both dependency waits and validated external prerequisite waits, with detail that identifies the blocker kind. It SHALL display `stalled` for no-progress or exhausted execution holds. When an iteration number applies, the display SHALL retain the `status:iteration` format.

#### Scenario: Dependency wait displays blocked

- **GIVEN** a change waits on an unarchived proposal dependency
- **WHEN** the TUI renders the change
- **THEN** its status is `blocked`
- **AND** its detail identifies a dependency blocker

#### Scenario: External prerequisite displays blocked

- **GIVEN** the orchestrator has validated an external prerequisite blocker
- **WHEN** the TUI renders the change
- **THEN** its status is `blocked`
- **AND** its detail exposes the external category, unblock condition, and next action

#### Scenario: Exhausted execution displays stalled

- **GIVEN** automatic execution stopped after no progress or retry exhaustion
- **WHEN** the TUI renders the change
- **THEN** its status is `stalled`
- **AND** the row is not described as waiting on a dependency or external prerequisite
