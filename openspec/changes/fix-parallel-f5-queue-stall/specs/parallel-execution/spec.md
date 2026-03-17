## MODIFIED Requirements

### Requirement: Parallel mode excludes uncommitted changes

Parallel mode SHALL exclude any change that is not present in `HEAD` or that has uncommitted or untracked files under `openspec/changes/<change_id>/` from execution.

When parallel start is triggered from the TUI, this authoritative exclusion result SHALL also reconcile the TUI row state so a rejected change is not left displayed as `Queued` after backend filtering.

#### Scenario: change with newly introduced uncommitted files is rejected at start

- **GIVEN** a change appeared eligible in the TUI based on the last refresh
- **AND** uncommitted or untracked files are introduced under `openspec/changes/<change_id>/` before the user presses `F5`
- **WHEN** parallel execution start performs its latest eligibility check
- **THEN** that change is excluded from execution
- **AND** the TUI does not leave the row in `Queued`
- **AND** the user can see a warning or log entry explaining the rejection
