## ADDED Requirements

### Requirement: Start-Time Queue Intent Ownership

The reducer SHALL own queued intent for TUI `F5` start and resume operations before any refresh-driven reconciliation occurs.

#### Scenario: Selected change keeps queued intent through startup reconciliation

- **GIVEN** the user has selected an eligible change in the TUI
- **AND** the user starts or resumes processing with `F5`
- **WHEN** the system performs its first reducer-driven display synchronization for that run
- **THEN** the shared orchestration state records queued intent for the selected change
- **AND** the displayed status remains `queued` until a later execution, wait, terminal, or rejection event changes it

### Requirement: Dependency Resolution Reveals Preserved Queue Intent

The reducer SHALL preserve queued intent while a queued change is dependency-blocked and SHALL restore the queued display when the dependency block clears.

#### Scenario: Dependency resolution returns blocked queued change to queued

- **GIVEN** a change already has queued intent in shared orchestration state
- **AND** that change is currently displayed as `blocked` because of unresolved dependencies
- **WHEN** the reducer processes `DependencyResolved` for that change
- **THEN** the dependency-block wait state is cleared
- **AND** the displayed status returns to `queued` without another user queue command
