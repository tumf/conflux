## MODIFIED Requirements

### Requirement: TUI Stopped Mode

The TUI SHALL provide a Stopped mode that manages change state by holding queued status only during execution. When transitioning to Stopped, `queue_status` SHALL be reset to `NotQueued` while preserving execution marks (`[x]`). Space operations in Stopped mode SHALL only add/remove execution marks while maintaining `queue_status` as `NotQueued`. When resuming with `F5`, execution-marked changes SHALL be revalidated against the latest execution constraints before they are treated as actively queued, and only startable changes SHALL remain queued while processing resumes. Task progress updates in Stopped mode SHALL NOT trigger queuing.

#### Scenario: Resume processing from Stopped mode

- **WHEN** the TUI is in Stopped mode
- **AND** one or more changes are execution-marked
- **AND** the user presses `F5`
- **THEN** the TUI transitions to Running mode
- **AND** processing resumes only for changes that still satisfy the latest start constraints
- **AND** any rejected change returns to `NotQueued`
- **AND** the log explains why the rejected change did not start
