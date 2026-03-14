# tui-error-handling Specification

## Purpose
TBD - created by archiving change update-tui-error-mode-continuation. Update Purpose after archive.
## Requirements
### Requirement: Change-Level Processing Errors Do Not Force App Error Mode

change の処理で `ProcessingError` が発生した場合、TUI は対象 change のステータスを `Error` として記録しなければならない（SHALL）。

このとき TUI 全体の AppMode は `Error` に遷移してはならない（SHALL NOT）。

#### Scenario: 処理中の change が失敗しても AppMode は維持される
- **GIVEN** the TUI is in running mode
- **AND** multiple changes are queued or processing
- **WHEN** a `ProcessingError` event is received for one change
- **THEN** the failed change SHALL transition to `Error`
- **AND** the AppMode SHALL remain `Running`

### Requirement: App Error Mode Is Reserved for Fatal Errors

TUI は致命的なエラーイベントを受信した場合にのみ AppMode を `Error` に遷移させなければならない（SHALL）。

#### Scenario: 致命的エラーで AppMode が Error になる
- **GIVEN** the TUI is running
- **WHEN** a fatal `Error` event is received
- **THEN** the AppMode SHALL transition to `Error`


#
