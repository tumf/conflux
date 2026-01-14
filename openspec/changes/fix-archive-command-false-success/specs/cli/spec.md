## MODIFIED Requirements

### Requirement: Reliable Archive Tracking

The TUI SHALL track archived changes reliably and report accurate final status.

#### Scenario: All changes archived successfully
- **WHEN** all queued changes have been processed and archived
- **THEN** the final verification reports "All processed changes have been archived"
- **AND** no unarchived warnings are displayed

#### Scenario: Archive failure handling
- **WHEN** an archive command fails for a change
- **THEN** the change is marked as errored
- **AND** the error is logged with details
- **AND** the change is not removed from tracking until explicitly handled

#### Scenario: Archive command succeeded but change not archived
- **WHEN** an archive command exits successfully (exit code 0)
- **AND** archive verification indicates the change is not archived
- **THEN** the orchestrator re-runs the archive command up to N times before marking the change as errored
- **AND** each retry attempt is logged
- **AND** no arbitrary delay-based polling is used
