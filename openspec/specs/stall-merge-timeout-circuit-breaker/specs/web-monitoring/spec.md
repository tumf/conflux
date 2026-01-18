## ADDED Requirements
### Requirement: Notification of Stop Due to Merge Stall

Web monitoring SHALL deliver state updates including the stop reason when a stop occurs due to merge stall.

#### Scenario: Stop occurs due to merge stall
- **GIVEN** web monitoring is enabled
- **AND** merge stall is detected
- **WHEN** a stop event is issued
- **THEN** the state update includes the merge stall stop reason
