## MODIFIED Requirements

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
