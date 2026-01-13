# tui-key-hints Specification

## Purpose
Defines TUI key binding hints display based on application mode.
## Requirements
### Requirement: Context-Aware Key Hints in Select Mode

The TUI SHALL display only actionable key hints based on current state in selection mode.

Changes panel title SHALL show only change-related keys.
App-level control keys SHALL be shown in Status panel title instead of Changes panel.

#### Scenario: Empty changes list hides selection keys

- **GIVEN** the TUI is in select mode
- **WHEN** the changes list is empty
- **THEN** the Changes panel key hints SHALL NOT show "Space: queue"
- **AND** the Changes panel key hints SHALL NOT show "@: approve"
- **AND** the Changes panel key hints SHALL NOT show "e: edit"
- **AND** the Changes panel key hints SHALL show "↑↓/jk: move"
- **AND** the Changes panel title SHALL NOT show "q: quit"

#### Scenario: No queued changes hides F5 key

- **GIVEN** the TUI is in select mode
- **AND** changes exist but none are selected for queue
- **THEN** the Changes panel key hints SHALL NOT show "F5: run"
- **AND** the Changes panel key hints SHALL show selection keys (Space/@/e)
- **AND** the Changes panel title SHALL NOT show "q: quit"

#### Scenario: Queued changes shows F5 key

- **GIVEN** the TUI is in select mode
- **AND** at least one change is selected for queue
- **THEN** the Changes panel key hints SHALL show "F5: run"
- **AND** the Changes panel title SHALL NOT show "q: quit"

### Requirement: Context-Aware Key Hints in Running Mode

The TUI SHALL display dynamic key hints in running mode consistent with select mode.

Changes panel title SHALL show only change-related keys.
App-level control keys (Esc, q) SHALL be shown in Status panel title instead of Changes panel.

#### Scenario: Running mode shows appropriate keys

- **GIVEN** the TUI is in running mode
- **WHEN** changes exist
- **THEN** the Changes panel key hints SHALL show selection keys based on current item state
- **AND** the Changes panel title SHALL NOT show "Esc: stop"
- **AND** the Changes panel title SHALL NOT show "q: quit"

#### Scenario: Running mode with empty list

- **GIVEN** the TUI is in running mode
- **WHEN** the changes list is empty
- **THEN** the Changes panel key hints SHALL NOT show selection keys
- **AND** the Changes panel title SHALL NOT show "Esc: stop"
- **AND** the Changes panel title SHALL NOT show "q: quit"

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

### Requirement: App Control Keys in Status Panel Title

The TUI SHALL display app-level control keys in the Status panel title based on current mode.

#### Scenario: Status panel title shows quit key in Select mode

- **GIVEN** the TUI is in select mode
- **THEN** the Status panel title SHALL show "q: quit"

#### Scenario: Status panel title shows stop and quit keys in Running mode

- **GIVEN** the TUI is in running mode
- **THEN** the Status panel title SHALL show "Esc: stop"
- **AND** the Status panel title SHALL show "q: quit"

#### Scenario: Status panel title shows force stop in Stopping mode

- **GIVEN** the TUI is in stopping mode
- **THEN** the Status panel title SHALL show "Esc: force stop"
- **AND** the Status panel title SHALL show "q: quit"

#### Scenario: Status panel title shows resume key in Stopped mode

- **GIVEN** the TUI is in stopped mode
- **THEN** the Status panel title SHALL show "F5: resume"
- **AND** the Status panel title SHALL show "q: quit"

### Requirement: Approval State Transition in Stopped Mode

TUIは停止モード（Stopped Mode）において、Running Mode と同様に承認時に自動的にキューへ追加しないものとする（SHALL）。

停止中は処理が行われないため、承認操作はキューステータスを変更すべきではない。

#### Scenario: Approve unapproved change without queuing in stopped mode

- **GIVEN** the TUI is in stopped mode (after pressing Esc to stop processing)
- **AND** the cursor is on an unapproved change showing `[ ]`
- **WHEN** the user presses `@`
- **THEN** the change SHALL become approved but NOT queued showing `[@]`
- **AND** a log message "Approved (not queued): {id}" SHALL appear
- **AND** the queue_status SHALL remain `NotQueued`

#### Scenario: Unapprove approved change in stopped mode

- **GIVEN** the TUI is in stopped mode
- **AND** the cursor is on an approved change showing `[@]`
- **WHEN** the user presses `@`
- **THEN** the change SHALL become unapproved showing `[ ]`
- **AND** a log message SHALL appear

#### Scenario: Unapprove queued change removes from queue in stopped mode

- **GIVEN** the TUI is in stopped mode
- **AND** the cursor is on an approved+queued change showing `[x]` (was queued before stop)
- **WHEN** the user presses `@`
- **THEN** the change SHALL become unapproved showing `[ ]`
- **AND** the change SHALL be removed from the queue
