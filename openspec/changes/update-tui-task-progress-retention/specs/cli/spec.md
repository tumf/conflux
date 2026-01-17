## MODIFIED Requirements

### Requirement: Running Footer Progress Bar Display

The running mode footer SHALL display a progress bar for overall processing progress.

#### Scenario: Completed task progress retention
- **WHEN** a change transitions to Completed, Archived, or Merged state
- **AND** the latest tasks.md is available at apply completion or archive start
- **THEN** the TUI preserves the last known task counts for that change
- **AND** the progress percentage does not decrease (monotonically increasing)
