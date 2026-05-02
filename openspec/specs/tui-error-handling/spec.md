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
