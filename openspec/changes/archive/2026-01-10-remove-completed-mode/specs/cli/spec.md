# cli Specification Delta

## ADDED Requirements

### Requirement: Log-based layout rendering

TUI layout SHALL be determined by log existence, not by mode.

#### Scenario: Select mode with no logs

- **GIVEN** TUI is in Select mode
- **AND** log entries are empty
- **WHEN** the screen renders
- **THEN** layout shows: Header + Changes list + Footer
- **AND** no log panel is displayed

#### Scenario: Select mode with logs

- **GIVEN** TUI is in Select mode
- **AND** log entries exist (from previous processing)
- **WHEN** the screen renders
- **THEN** layout shows: Header + Changes list + Status + Logs
- **AND** log panel displays existing log entries

#### Scenario: Return to Select mode after completion

- **GIVEN** TUI is in Running mode
- **AND** all queued changes have been processed
- **WHEN** the last change completes
- **THEN** TUI transitions to Select mode
- **AND** completion message is added to logs
- **AND** log panel remains visible (logs exist)

