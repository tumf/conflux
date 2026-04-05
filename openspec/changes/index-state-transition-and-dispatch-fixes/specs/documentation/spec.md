## ADDED Requirements

### Requirement: State transition consistency tracking

The project SHALL maintain an index proposal that tracks structural inconsistencies across state models (WorkspaceState, WorkspaceStatus, ChangeRuntimeState) and the parallel scheduler dispatch logic, and coordinates their resolution through phased sub-proposals.

#### Scenario: Index proposal tracks sub-proposal completion

- **GIVEN** the index proposal lists 4 sub-proposals with execution phases
- **WHEN** all sub-proposals are archived
- **THEN** the index proposal acceptance criteria are met
- **AND** the index proposal can be archived
