## MODIFIED Requirements

### Requirement: Resolve Wait Queue Ownership

The system SHALL treat `ResolveMerge` / `MergeWait` retry as reducer-owned scheduler intent, not as a TUI-local direct execution operation.

When the scheduler retries an archived Git merge and the merge path reaches a normal merge-ready state without unresolved conflicts, the runtime SHALL complete that merge through the normal merge/verification path and SHALL NOT start AI conflict resolution solely because the retry entered the resolve-capable code path.

Post-merge verification for this path SHALL accept repository-visible merge success without requiring the archived source branch tip to continue containing the pre-merge base after the merge commit has already integrated the change into the target branch.

#### Scenario: Conflictless archived merge does not emit resolve command

- **GIVEN** change `alpha` is archived and reaches scheduler-owned merge retry
- **AND** the target branch merge preparation succeeds without unresolved conflicts
- **AND** conflict detection returns no conflict files
- **WHEN** the runtime evaluates whether to start conflict resolution
- **THEN** it SHALL NOT emit `ResolveStarted` for `alpha`
- **AND** it SHALL NOT build a conflict-oriented resolve prompt for `alpha`
- **AND** it SHALL continue through the normal merge completion path

#### Scenario: Successful merge commit is not retried for false pre-sync negative

- **GIVEN** change `alpha` is archived and merged into the target branch by a valid merge commit
- **AND** the archived source branch tip itself no longer proves inclusion of the pre-merge base
- **WHEN** post-merge verification runs
- **THEN** the runtime SHALL accept the merged outcome from repository-visible merge evidence
- **AND** it SHALL NOT retry resolve solely because the source branch tip does not include the pre-merge base

#### Scenario: True conflict still enters resolve path

- **GIVEN** change `alpha` is archived and reaches scheduler-owned merge retry
- **AND** the target branch merge preparation leaves unresolved conflicts
- **WHEN** the runtime evaluates conflict resolution
- **THEN** it SHALL emit `ResolveStarted` for `alpha`
- **AND** the resolve prompt SHALL include non-empty conflict evidence
