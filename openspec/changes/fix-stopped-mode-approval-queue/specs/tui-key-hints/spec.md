## ADDED Requirements

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
