## ADDED Requirements

### Requirement: Initial Parallel Refresh Preserves Startup Queue State

The parallel TUI startup sequence SHALL preserve selected queued targets across its initial refresh and SHALL only clear rows that the backend explicitly rejects.

#### Scenario: Initial parallel refresh does not regress queued startup row

- **GIVEN** the user starts parallel processing from the TUI with a selected eligible change
- **WHEN** the parallel startup path emits its initial `ChangesRefreshed` event before that change begins execution
- **THEN** the change remains `queued` in the TUI
- **AND** the shared orchestration state still records queued intent for that change

#### Scenario: Parallel start rejection clears only rejected IDs

- **GIVEN** the user starts parallel processing with multiple selected changes
- **AND** the backend rejects only a subset of those change IDs at parallel start time
- **WHEN** the rejection is applied to TUI and shared orchestration state
- **THEN** only the rejected change IDs return to `not queued`
- **AND** every non-rejected selected change remains `queued` or `blocked` according to its actual reducer state
