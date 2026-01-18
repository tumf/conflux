## NEW Requirements
### Requirement: REQ-OBS-005 TUI Input Rejection Logging

The orchestrator MUST log warning messages when user input is ignored in the TUI to help users understand why their actions had no effect.

#### Scenario: Enter Key Ignored in Worktrees View

- **GIVEN** the TUI is displaying the Worktrees view
- **WHEN** the Enter key is pressed but ignored due to missing conditions
- **THEN** a warning log is displayed with a message explaining the rejection reason
- **AND** the message enables the user to determine the required conditions
