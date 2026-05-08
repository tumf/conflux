# tui-error-handling Specification

## Purpose
TBD - created by archiving change update-tui-error-mode-continuation. Update Purpose after archive.
## Requirements

### Requirement: Change-Level Processing Errors Do Not Force App Error Mode

change の処理で `ProcessingError` が発生した場合、TUI は対象 change のステータスを `Error` として記録しなければならない（SHALL）。

このとき TUI 全体の AppMode は `Error` に遷移してはならない（SHALL NOT）。

Non-fatal warning popups used for merge, resolve, hook, and warning diagnostics SHALL preserve readable diagnostic content. When a warning popup message contains explicit newlines, the popup SHALL preserve those line boundaries. When warning popup content exceeds the visible body area, the TUI SHALL provide popup-local scrolling and SHALL NOT route popup scroll keys to the underlying change list or log panel. Warning popup presentation state SHALL NOT be used as workflow-control input.

#### Scenario: 処理中の change が失敗しても AppMode は維持される

- **GIVEN** the TUI is in running mode
- **AND** multiple changes are queued or processing
- **WHEN** a `ProcessingError` event is received for one change
- **THEN** the failed change SHALL transition to `Error`
- **AND** the AppMode SHALL remain `Running`

#### Scenario: on_merged hook failure popup preserves multi-line diagnostics

- **GIVEN** the TUI receives an `on_merged` hook failure for change `change-a`
- **AND** the failure error contains newline-separated diagnostics
- **WHEN** the warning popup is shown
- **THEN** the popup message SHALL include the newline-separated diagnostics without collapsing them into a single unreadable line
- **AND** the warning log entry SHALL still include the failure message

#### Scenario: Warning popup supports modal-local scrolling

- **GIVEN** a warning popup is visible
- **AND** its message is longer than the visible popup body
- **WHEN** the user presses a popup scroll key such as `Down`, `j`, or `PageDown`
- **THEN** the popup SHALL remain visible
- **AND** the popup content SHALL scroll within the popup
- **AND** the underlying change cursor and log scroll SHALL NOT move because of that key press

#### Scenario: Warning popup closes with explicit close key

- **GIVEN** a warning popup is visible
- **WHEN** the user presses `Esc`
- **THEN** the warning popup SHALL close
- **AND** no workflow state transition SHALL be caused by closing the popup

### Requirement: App Error Mode Is Reserved for Fatal Errors

TUI は致命的なエラーイベントを受信した場合にのみ AppMode を `Error` に遷移させなければならない（SHALL）。

Stale local auto-refresh roots, including a repository/worktree path deleted after the TUI captured it, SHALL NOT be treated as change-processing errors or fatal AppMode errors. The TUI SHALL bound warnings for such stale roots so the same missing root does not produce repeated snapshot warnings on every refresh tick.

#### Scenario: 致命的エラーで AppMode が Error になる
- **GIVEN** the TUI is running
- **WHEN** a fatal `Error` event is received
- **THEN** the AppMode SHALL transition to `Error`

#### Scenario: Missing auto-refresh root does not flood warnings

- **GIVEN** a local TUI session captured a repository/worktree root
- **AND** that root no longer exists when auto-refresh runs
- **WHEN** multiple auto-refresh ticks occur
- **THEN** the TUI SHALL NOT run repeated snapshot git commands against the missing root on every tick
- **AND** the TUI SHALL emit at most one warning for that stale root per session, or apply an explicit rate limit/backoff
- **AND** AppMode SHALL NOT transition to `Error` solely because of the stale refresh root

#### Scenario: Existing-root snapshot failures remain visible

- **GIVEN** a local TUI session captured a repository/worktree root that still exists
- **AND** a refresh snapshot command fails for an actionable git/VCS reason
- **WHEN** auto-refresh handles the failure
- **THEN** the TUI SHALL log a warning with enough context to identify the failed snapshot operation and root
