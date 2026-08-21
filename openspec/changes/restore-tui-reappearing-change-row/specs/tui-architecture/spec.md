## ADDED Requirements

### Requirement: Refresh projection convergence

The local TUI change projection MUST converge to the current successful active and rejected change observations. If a change is temporarily absent from one refresh and is observed again later, stale identity bookkeeping MUST NOT suppress reconstruction of its row.

Reappearance state and logs are observability-only. They MUST NOT become inputs to queue membership, scheduler dispatch, resume routing, acceptance, archive routing, or other next-action decisions.

<!-- Expected canonical result after archive: `tui-architecture` will require change rows to recover after transient refresh absence without mutating workflow intent. -->

#### Scenario: Active proposal reappears after transient absence

- **GIVEN** an active proposal row is present in the local TUI
- **AND** a later successful refresh temporarily does not observe that proposal
- **WHEN** a subsequent successful refresh observes the active proposal again
- **THEN** the TUI reconstructs and displays its row from the current refresh data
- **AND** stale known-ID bookkeeping does not suppress the row
- **AND** the row receives the existing newly-detected observability treatment

#### Scenario: Reappearance preserves operator intent and cursor

- **GIVEN** an active proposal reappears after a transient absence
- **WHEN** the TUI refreshes its projection
- **THEN** the cursor is not moved solely because of the reappearance
- **AND** the proposal is not automatically selected, marked, queued, dispatched, resumed, accepted, or archived
- **AND** reducer and repository state remain authoritative for workflow behavior

#### Scenario: Rejected proposal remains read-only

- **GIVEN** a rejected proposal is absent from one refresh and observed again later
- **WHEN** the TUI reconstructs its row
- **THEN** the row is displayed as rejected and read-only
- **AND** it does not receive an active NEW badge or increment the active new-change counter
