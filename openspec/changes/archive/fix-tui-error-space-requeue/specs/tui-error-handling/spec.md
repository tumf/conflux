## MODIFIED Requirements

### Requirement: Change-Level Processing Errors Do Not Force App Error Mode

change の処理で `ProcessingError` が発生した場合、TUI は対象 change のステータスを `Error` として記録しなければならない（SHALL）。

このとき TUI 全体の AppMode は `Error` に遷移してはならない（SHALL NOT）。

また、対象 change の execution mark (`selected`) は clear されなければならない（SHALL）。

#### Scenario: 処理中の change が失敗しても AppMode は維持される
- **GIVEN** the TUI is in running mode
- **AND** multiple changes are queued or processing
- **WHEN** a `ProcessingError` event is received for one change
- **THEN** the failed change SHALL transition to `Error`
- **AND** the AppMode SHALL remain `Running`
- **AND** the failed change's `selected` SHALL become `false`
