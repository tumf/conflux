## MODIFIED Requirements

### Requirement: Git Detection at TUI Startup

The local executable TUI SHALL verify repository identity, Git command availability, and that the current workspace is the repository's main worktree before repository-lock acquisition or orchestration startup. It SHALL reject bare `cflx` and `cflx tui` startup from linked worktrees with an actionable main-worktree diagnostic. It SHALL NOT silently redirect to another worktree or degrade to serial execution.

#### Scenario: Git repository is usable at startup

- **GIVEN** user starts the local TUI in the repository's main worktree
- **AND** the Git command is available
- **WHEN** startup validation completes
- **THEN** worktree orchestration controls are available
- **AND** no execution-mode toggle is displayed

#### Scenario: Git repository is unavailable at startup

- **GIVEN** user starts the local executable TUI outside a repository or without the Git command
- **WHEN** startup validation runs
- **THEN** startup fails with an actionable error before repository-lock acquisition or other orchestration side effects
- **AND** the TUI does not offer a serial fallback

#### Scenario: TUI starts from a linked worktree

- **GIVEN** user starts bare `cflx` or `cflx tui` in a linked worktree
- **WHEN** startup validation runs
- **THEN** startup fails before the repository lock is acquired or the terminal is taken over
- **AND** the error names the current linked worktree and the repository's main worktree
- **AND** the error instructs the user to start Conflux from the main worktree
- **AND** no automatic redirect, worktree cleanup, or orchestration side effect occurs
