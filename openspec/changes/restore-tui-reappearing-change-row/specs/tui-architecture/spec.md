## ADDED Requirements

### Requirement: Refresh projection convergence

The local TUI MUST display a row for every change present in the current successful active and rejected observations, and its change identity bookkeeping MUST converge to the row projection that survives each successful refresh: an identity entry MUST NOT outlive its row, and an entry whose row is deliberately retained through a snapshot absence (recorded start, or terminal/wait display status) MUST stay known. If a change's row was removed during a transient absence and the change is observed again later, stale identity bookkeeping MUST NOT suppress reconstruction of its row; if the row was retained through the absence, re-observation MUST update it in place without creating a duplicate row.

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

#### Scenario: Retained row is re-observed without duplication

- **GIVEN** a change row is retained through a snapshot absence because it has a recorded start or a terminal/wait display status
- **WHEN** a subsequent successful refresh observes that change again
- **THEN** the existing row is updated in place from the current refresh data
- **AND** no duplicate row is added for the same change ID
- **AND** the row does not receive the newly-detected badge and no detection log is emitted for it

#### Scenario: Rejected proposal remains read-only

- **GIVEN** a proposal's row was removed during a transient absence and a later successful refresh observes the proposal as rejected
- **WHEN** the TUI reconstructs its row
- **THEN** the row is displayed as rejected and read-only
- **AND** it does not receive an active NEW badge or increment the active new-change counter
