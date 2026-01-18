## ADDED Requirements
### Requirement: Display Stop Due to Merge Stall

The TUI SHALL display the stop reason when a stop occurs due to merge stall.

#### Scenario: Merge stall detected during execution
- **GIVEN** TUI is in run mode
- **AND** merge stall is detected
- **WHEN** the orchestrator stops
- **THEN** the TUI displays the stop reason due to merge stall
