# cli Specification Delta

## MODIFIED Requirements

### Requirement: TUI Approval Toggle

The TUI SHALL allow users to toggle approval status using the `@` key, with different auto-queue behavior based on orchestrator state.

#### Scenario: Approve unapproved change in Running mode (approve only)

- **WHEN** TUI is in Running mode (orchestrator actively processing)
- **AND** user presses `@` key on an unapproved change (`[ ]`)
- **THEN** the change becomes approved but NOT queued (`[@]`)
- **AND** checkbox transitions from `[ ]` to `[@]`
- **AND** log message indicates approval only

#### Scenario: Approve unapproved change in Select mode adds to queue automatically

- **WHEN** TUI is in Select mode (orchestrator stopped)
- **AND** user presses `@` key on an unapproved change (`[ ]`)
- **THEN** the change becomes approved AND queued (`[x]`)
- **AND** checkbox transitions directly from `[ ]` to `[x]`
- **AND** log message indicates both approval and queue addition

#### Scenario: Approve unapproved change in Completed mode adds to queue automatically

- **WHEN** TUI is in Completed mode (orchestrator stopped, all queued changes done)
- **AND** user presses `@` key on an unapproved change (`[ ]`)
- **THEN** the change becomes approved AND queued (`[x]`)
- **AND** checkbox transitions directly from `[ ]` to `[x]`
- **AND** log message indicates both approval and queue addition

#### Scenario: Unapprove approved-but-not-queued change

- **WHEN** TUI is in any mode (Select, Running, or Completed)
- **AND** user presses `@` key on an approved but not queued change (`[@]`)
- **THEN** the change becomes unapproved (`[ ]`)
- **AND** checkbox transitions from `[@]` to `[ ]`

#### Scenario: Unapprove queued change removes from queue

- **WHEN** TUI is in any mode (Select, Running, or Completed)
- **AND** user presses `@` key on a queued change (`[x]`) that is NOT processing
- **THEN** the change becomes unapproved AND removed from queue (`[ ]`)
- **AND** checkbox transitions from `[x]` to `[ ]`
- **AND** log message indicates both unapproval and queue removal

#### Scenario: Toggle approval blocked for processing change

- **WHEN** TUI is in Running mode
- **AND** user presses `@` key
- **AND** highlighted change is in `Processing` state
- **THEN** approval status is NOT changed
- **AND** a warning message is displayed: "Cannot change approval for processing change"
