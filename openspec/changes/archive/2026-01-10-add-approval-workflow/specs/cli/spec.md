# cli Specification Delta

## ADDED Requirements

### Requirement: approve Subcommand

The CLI SHALL provide an `approve` subcommand to manage change approval status.

#### Scenario: Approve a change with set action

- **WHEN** user runs `openspec-orchestrator approve set {change_id}`
- **AND** the change directory `openspec/changes/{change_id}/` exists
- **THEN** an `approved` file is created in the change directory
- **AND** the file contains MD5 checksums of all `.md` files (except `tasks.md`)
- **AND** a success message is displayed

#### Scenario: Approve a change that doesn't exist

- **WHEN** user runs `openspec-orchestrator approve set {change_id}`
- **AND** the change directory does not exist
- **THEN** an error message is displayed
- **AND** exit code is non-zero

#### Scenario: Unapprove a change with unset action

- **WHEN** user runs `openspec-orchestrator approve unset {change_id}`
- **AND** the `approved` file exists
- **THEN** the `approved` file is deleted
- **AND** a success message is displayed

#### Scenario: Unapprove a change that is not approved

- **WHEN** user runs `openspec-orchestrator approve unset {change_id}`
- **AND** the `approved` file does not exist
- **THEN** a message indicates the change was not approved
- **AND** exit code is zero (no-op)

#### Scenario: Check approval status

- **WHEN** user runs `openspec-orchestrator approve status {change_id}`
- **THEN** the approval status is displayed
- **AND** if approved, shows "approved" with file count
- **AND** if not approved, shows reason (file missing, hash mismatch, etc.)

### Requirement: TUI Approval Toggle

The TUI SHALL allow users to toggle approval status using the `@` key.

#### Scenario: Toggle approval in selection mode

- **WHEN** TUI is in selection mode
- **AND** user presses `@` key
- **AND** a change is highlighted
- **THEN** the approval status of the highlighted change is toggled
- **AND** if previously unapproved, an `approved` file is created
- **AND** if previously approved, the `approved` file is deleted

#### Scenario: Approval badge display

- **WHEN** TUI displays the change list
- **AND** a change is approved
- **THEN** an `@` badge is displayed next to the change ID
- **AND** the badge uses a distinct color (green recommended)

#### Scenario: Help text includes approval key

- **WHEN** TUI is in selection mode
- **THEN** the help text includes `@: approve` in the key bindings

### Requirement: Auto-Queue Approved Changes on TUI Startup

The TUI SHALL automatically queue approved changes when starting in TUI mode.

#### Scenario: TUI startup with approved changes

- **WHEN** user starts the TUI
- **AND** one or more changes have valid `approved` files
- **THEN** those changes are automatically selected and queued
- **AND** a log message indicates "Auto-queued N approved changes"

#### Scenario: TUI startup with no approved changes

- **WHEN** user starts the TUI
- **AND** no changes have valid `approved` files
- **THEN** no changes are automatically queued
- **AND** the user can manually select and approve changes

### Requirement: Unapproved Changes Cannot Be Queued

The system SHALL prevent unapproved changes from being added to the execution queue.

#### Scenario: Attempt to queue unapproved change in TUI

- **WHEN** TUI is in selection mode
- **AND** user presses Space to select an unapproved change
- **THEN** the change can be selected for viewing
- **AND** pressing F5 with only unapproved changes selected shows warning
- **AND** the warning suggests approving changes first

#### Scenario: CLI run with unapproved change

- **WHEN** user runs `openspec-orchestrator run --change {change_id}`
- **AND** the change is not approved
- **THEN** a warning message is displayed
- **AND** the change is NOT added to the queue
- **AND** processing continues with any remaining approved changes

#### Scenario: CLI run with mixed approved/unapproved changes

- **WHEN** user runs `openspec-orchestrator run --change a,b,c`
- **AND** change `a` is approved, `b` is not approved, `c` is approved
- **THEN** warning is displayed for change `b`
- **AND** only changes `a` and `c` are processed
