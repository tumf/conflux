## MODIFIED Requirements
### Requirement: Reliable Archive Tracking

The archive verification SHALL treat a change as unarchived when `openspec/changes/{change_id}` exists.

#### Scenario: changes directory still present after archive
- **WHEN** the archive command succeeds
- **AND** `openspec/changes/{change_id}` still exists
- **THEN** archive verification is treated as unarchived
- **AND** the archive command is re-run
