## ADDED Requirements

### Requirement: F5 Synchronizes Shared Queue Intent

The TUI SHALL synchronize reducer-owned queue intent when `F5` starts or resumes processing, rather than relying only on TUI-local queue status.

#### Scenario: Select-mode F5 queues selected changes in shared state

- **GIVEN** the TUI is in Select mode
- **AND** one or more eligible changes are selected
- **WHEN** the user presses `F5`
- **THEN** the TUI records queued intent for those changes in shared orchestration state
- **AND** the TUI transitions to Running mode using that shared queued state

#### Scenario: Stopped-mode F5 restores execution-marked changes in shared state

- **GIVEN** the TUI is in Stopped mode
- **AND** one or more changes are execution-marked
- **WHEN** the user presses `F5`
- **THEN** the TUI records queued intent for those execution-marked changes in shared orchestration state
- **AND** the resumed run displays those changes as `queued` until later execution state changes occur
