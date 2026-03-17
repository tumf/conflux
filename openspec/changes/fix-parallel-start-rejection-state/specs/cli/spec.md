## MODIFIED Requirements

### Requirement: TUI Stopped Mode

The TUI SHALL provide a Stopped mode that manages change state by holding queued status only during execution. When transitioning to Stopped, `queue_status` SHALL be reset to `NotQueued` while preserving execution marks (`[x]`). Space operations in Stopped mode SHALL only add/remove execution marks while maintaining `queue_status` as `NotQueued`. When resuming with `F5`, execution-marked changes SHALL be revalidated against the latest execution constraints before they are treated as actively queued, and any change rejected by backend start-time filtering SHALL return to `NotQueued` with an explanatory warning or log entry. Task progress updates in Stopped mode SHALL NOT trigger queuing.

#### Scenario: Resume processing from Stopped mode rejects newly uncommitted change

- **WHEN** the TUI is in Stopped mode
- **AND** one or more changes are execution-marked
- **AND** one marked change becomes uncommitted before the user presses `F5`
- **AND** the user presses `F5`
- **THEN** processing resumes only for changes that still satisfy the latest start constraints
- **AND** the rejected change returns to `NotQueued`
- **AND** the log explains why the rejected change did not start

### Requirement: Run command parallel start reporting

The `cflx run --parallel` command SHALL report whether backend start-time eligibility filtering prevented requested changes from starting.

#### Scenario: parallel run rejects all requested changes before start

- **GIVEN** the user runs `cflx run --parallel` for one or more requested changes
- **AND** backend start-time eligibility filtering rejects all of them as uncommitted or otherwise not startable
- **WHEN** the run command handles the rejection result
- **THEN** the CLI clearly reports that no changes started
- **AND** the CLI explains the rejection reason instead of implying normal execution completion
