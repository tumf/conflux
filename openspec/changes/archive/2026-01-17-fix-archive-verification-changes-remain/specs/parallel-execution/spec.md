## MODIFIED Requirements
### Requirement: Individual Merge on Archive Completion

In parallel execution mode, the system SHALL re-check `verify_archive_completion` before starting a merge. If `openspec/changes/{change_id}` exists, it is treated as unarchived, `MergeDeferred` is returned, and the change remains in `MergeWait`.

#### Scenario: merge deferred when changes directory still exists
- **GIVEN** change A was processed as archived
- **AND** `openspec/changes/{change_id}` still exists
- **WHEN** the merge begins
- **THEN** `verify_archive_completion` reports unarchived
- **AND** `MergeDeferred` is returned
- **AND** change A remains in `MergeWait`
