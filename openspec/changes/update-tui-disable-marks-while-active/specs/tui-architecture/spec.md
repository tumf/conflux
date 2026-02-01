## ADDED Requirements
### Requirement: Active Change Input Lockout

`queue_status.is_active()` が true の change では、Space(@/x) と @ の操作を受け付けてはならない（MUST NOT）。
このとき、選択状態・承認状態・`queue_status`・DynamicQueue は変更されてはならない（MUST NOT）。

#### Scenario: is_active中はSpace操作を拒否する
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with `queue_status.is_active() == true`
- **WHEN** the user presses Space
- **THEN** the selection mark SHALL remain unchanged
- **AND** the queue status SHALL remain unchanged
- **AND** DynamicQueue SHALL NOT be modified

#### Scenario: is_active中は@操作を拒否する
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with `queue_status.is_active() == true`
- **WHEN** the user presses `@`
- **THEN** the approval state SHALL remain unchanged
- **AND** the queue status SHALL remain unchanged
- **AND** DynamicQueue SHALL NOT be modified
