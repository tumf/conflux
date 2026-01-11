# tui-key-hints Specification

## Purpose
TBD - created by archiving change improve-dynamic-key-hints. Update Purpose after archive.
## Requirements
### Requirement: Context-Aware Key Hints in Select Mode

The TUI SHALL display only actionable key hints based on current state in selection mode.

#### Scenario: Empty changes list hides selection keys

- **GIVEN** the TUI is in select mode
- **WHEN** the changes list is empty
- **THEN** the key hints SHALL NOT show "Space: queue"
- **AND** the key hints SHALL NOT show "@: approve"
- **AND** the key hints SHALL NOT show "e: edit"
- **AND** the key hints SHALL show "↑↓/jk: move"
- **AND** the key hints SHALL show "q: quit"

#### Scenario: No queued changes hides F5 key

- **GIVEN** the TUI is in select mode
- **AND** changes exist but none are selected for queue
- **THEN** the key hints SHALL NOT show "F5: run"
- **AND** the key hints SHALL show selection keys (Space/@/e)

#### Scenario: Queued changes shows F5 key

- **GIVEN** the TUI is in select mode
- **AND** at least one change is selected for queue
- **THEN** the key hints SHALL show "F5: run"

### Requirement: Context-Aware Key Hints in Running Mode

The TUI SHALL display dynamic key hints in running mode consistent with select mode.

#### Scenario: Running mode shows appropriate keys

- **GIVEN** the TUI is in running mode
- **WHEN** changes exist
- **THEN** the key hints SHALL show selection keys based on current item state
- **AND** the key hints SHALL show "Esc: stop"
- **AND** the key hints SHALL show "q: quit"

#### Scenario: Running mode with empty list

- **GIVEN** the TUI is in running mode
- **WHEN** the changes list is empty
- **THEN** the key hints SHALL NOT show selection keys
- **AND** the key hints SHALL show "Esc: stop"
- **AND** the key hints SHALL show "q: quit"

### Requirement: Approval State Transition in Select Mode

The TUI SHALL transition change state correctly when pressing `@` key in select mode.

#### Scenario: Approve unapproved change adds to queue

- **GIVEN** the TUI is in select mode
- **AND** the cursor is on an unapproved change showing `[ ]`
- **WHEN** the user presses `@`
- **THEN** the change SHALL become approved AND queued showing `[x]`
- **AND** a log message "Approved and queued: {id}" SHALL appear
- **AND** the logs panel SHALL become visible

#### Scenario: Unapprove approved change removes from queue

- **GIVEN** the TUI is in select mode
- **AND** the cursor is on an approved+queued change showing `[x]`
- **WHEN** the user presses `@`
- **THEN** the change SHALL become unapproved showing `[ ]`
- **AND** the change SHALL be removed from the queue
- **AND** a log message SHALL appear

### Requirement: Approval State Transition in Running Mode

The TUI SHALL allow approval without auto-queuing in running mode.

#### Scenario: Approve unapproved change without queuing

- **GIVEN** the TUI is in running mode
- **AND** the cursor is on an unapproved change showing `[ ]`
- **WHEN** the user presses `@`
- **THEN** the change SHALL become approved but NOT queued showing `[@]`
- **AND** a log message "Approved (not queued): {id}" SHALL appear
