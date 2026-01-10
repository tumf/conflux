# cli Specification Delta

## NEW Requirements

### Requirement: Running Mode Change Display

The TUI SHALL display checkbox indicators in running/completed mode to show approval and queue status.

#### Scenario: Checkbox display in running mode

- **WHEN** TUI is in running mode
- **AND** change list is rendered
- **THEN** each change displays a checkbox indicator before the cursor
- **AND** unapproved changes show `[ ]` in dark gray
- **AND** approved non-queued changes show `[@]` in yellow
- **AND** queued/processing/completed/archived changes show `[x]` in green

#### Scenario: Checkbox display in completed mode

- **WHEN** TUI is in completed mode
- **AND** change list is rendered
- **THEN** each change displays the same checkbox indicators as running mode

### Requirement: Processing Change Protection

The TUI SHALL prevent any status modification for changes that are currently being processed.

#### Scenario: Cannot remove processing change from queue

- **WHEN** TUI is in running mode
- **AND** user presses Space key on a `Processing` change
- **THEN** the queue status is NOT changed
- **AND** the change remains in `Processing` state

#### Scenario: Cannot unapprove processing change

- **WHEN** TUI is in running mode
- **AND** user presses `@` key on a `Processing` change
- **THEN** the approval status is NOT changed
- **AND** a warning message is displayed: "Cannot change approval for processing change"

#### Scenario: Processing change indicators are locked

- **WHEN** a change is in `Processing` state
- **THEN** its checkbox always displays `[x]` (queued/processing indicator)
- **AND** the checkbox color reflects the processing status (cyan)

#### Scenario: Toggle approval in completed mode

- **WHEN** TUI is in completed mode
- **AND** user presses `@` key
- **AND** a change is highlighted
- **THEN** the approval status of the highlighted change is toggled
- **AND** the checkbox indicator updates accordingly

### Requirement: Dynamic Queue Help Text

The TUI SHALL display appropriate help text for running/completed mode.

#### Scenario: Help text in running mode

- **WHEN** TUI is in running mode
- **THEN** the changes panel title includes `@: approve` in the key bindings
- **AND** displays "(Space: add/remove from queue, @: approve)"

## MODIFIED Requirements

### Requirement: TUI Approval Toggle

The TUI SHALL allow users to toggle approval status using the `@` key.

#### Scenario: Approve unapproved change adds to queue automatically

- **WHEN** TUI is in selection mode or running mode
- **AND** user presses `@` key on an unapproved change (`[ ]`)
- **THEN** the change becomes approved AND queued (`[x]`)
- **AND** checkbox transitions directly from `[ ]` to `[x]`
- **AND** log message indicates both approval and queue addition

#### Scenario: Unapprove approved-but-not-queued change

- **WHEN** TUI is in selection mode or running mode
- **AND** user presses `@` key on an approved but not queued change (`[@]`)
- **THEN** the change becomes unapproved (`[ ]`)
- **AND** checkbox transitions from `[@]` to `[ ]`

#### Scenario: Unapprove queued change removes from queue

- **WHEN** TUI is in selection mode or running mode
- **AND** user presses `@` key on a queued change (`[x]`) that is NOT processing
- **THEN** the change becomes unapproved AND removed from queue (`[ ]`)
- **AND** checkbox transitions from `[x]` to `[ ]`
- **AND** log message indicates both unapproval and queue removal

#### Scenario: Toggle approval blocked for processing change

- **WHEN** TUI is in running mode
- **AND** user presses `@` key
- **AND** highlighted change is in `Processing` state
- **THEN** approval status is NOT changed
- **AND** a warning message is displayed: "Cannot change approval for processing change"
