## MODIFIED Requirements
### Requirement: Interrupted Change Handling
Changes interrupted by stop SHALL be handled according to the policy of holding queued status only during execution. When force-stopped, queue_status SHALL be reset to NotQueued while preserving execution marks. On resume, execution-marked changes SHALL be restored to queued and can be re-processed. Accepting status SHALL be treated as an in-flight execution state and MUST be reset to NotQueued when the user force-stops.

#### Scenario: Force-stopped accepting change returns to not queued
- **GIVEN** a change is in Accepting status
- **WHEN** the user force stops with second Esc press
- **THEN** the change status becomes not queued
- **AND** the execution mark remains set
