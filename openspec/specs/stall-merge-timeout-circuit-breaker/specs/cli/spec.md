## ADDED Requirements
### Requirement: Force Stop Message Due to Merge Stall

The CLI SHALL display a message indicating the stop reason when a force stop occurs due to merge stall.

#### Scenario: Merge stall detected in run mode
- **GIVEN** `cflx run` is executing
- **AND** merge stall is detected
- **WHEN** the orchestrator stops
- **THEN** the CLI displays a stop message due to merge stall
