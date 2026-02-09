## MODIFIED Requirements
### Requirement: Active Change Input Lockout
`queue_status.is_active()` が true の change では、`@` 操作を受け付けてはならない（MUST NOT）。
`Space` 操作は単体停止要求として受け付けなければならない（SHALL）。`Space` 操作は即時に `selected`/`queue_status` を変更してはならず（SHALL NOT）、停止完了イベントを受信した時点で `selected=false` と `not queued` に遷移させなければならない（SHALL）。

#### Scenario: active change を Space で単体停止する
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with `queue_status.is_active() == true`
- **WHEN** the user presses Space
- **THEN** a stop request for that change is issued
- **AND** the change remains in its active queue_status until stop completion
- **AND** after `ChangeStopped` is processed, the change becomes `not queued` and `selected` is cleared

#### Scenario: active change で @ 操作は無効
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change with `queue_status.is_active() == true`
- **WHEN** the user presses `@`
- **THEN** the approval state remains unchanged
- **AND** the queue_status remains unchanged
