## MODIFIED Requirements
### Requirement: TUI Help Text for Stop

The TUI help text SHALL include stop key binding information.

#### Scenario: Stopping mode help text
- **WHEN** TUI is in Stopping mode
- **THEN** help text includes "Esc: force stop"
- **AND** help text includes "F5: continue"
- **AND** help text shows "Waiting for current process..."
