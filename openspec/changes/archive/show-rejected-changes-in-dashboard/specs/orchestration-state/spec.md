## MODIFIED Requirements

### Requirement: Rejected Change Exclusion from Change Listing

The system SHALL treat `openspec/changes/<change_id>/REJECTED.md` as the durable rejection marker and SHALL exclude marker-bearing changes from the execution-oriented active listing returned by `list_changes_native()`.

This exclusion contract applies to execution candidate discovery and queue addition. It SHALL NOT by itself forbid read-only operational surfaces such as the dashboard change list from showing the rejected change as a terminal status row.

#### Scenario: Rejected marker excludes change from active execution list

- **GIVEN** `openspec/changes/fix-auth/REJECTED.md` exists
- **AND** `openspec/changes/fix-auth/proposal.md` exists
- **WHEN** `list_changes_native()` is called
- **THEN** `fix-auth` is NOT included in the returned change list

#### Scenario: Dashboard may still show rejected change

- **GIVEN** `openspec/changes/fix-auth/REJECTED.md` exists
- **AND** `openspec/changes/fix-auth/proposal.md` exists
- **WHEN** a dashboard-facing change snapshot is built
- **THEN** `fix-auth` MAY be included as a read-only rejected row
- **AND** the execution-oriented active listing remains unchanged
