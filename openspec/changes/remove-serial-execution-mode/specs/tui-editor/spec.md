## REMOVED Requirements

### Requirement: Parallel Mode Toggle Key

Removed because worktree orchestration is the only execution model and cannot be toggled off.

### Requirement: Parallel Mode State Indicator

Removed because a badge distinguishing the sole execution model carries no state information.

### Requirement: Parallel Mode Toggle During Modes

Removed because no app mode accepts an execution-mode mutation.

## MODIFIED Requirements

### Requirement: Git Detection at TUI Startup

The local executable TUI SHALL verify repository identity and Git command availability before orchestration can start. It SHALL NOT silently degrade to serial execution.

#### Scenario: Git repository is usable at startup

- **GIVEN** user starts the local TUI in a Git repository
- **AND** the Git command is available
- **WHEN** startup validation completes
- **THEN** worktree orchestration controls are available
- **AND** no execution-mode toggle is displayed

#### Scenario: Git repository is unavailable at startup

- **GIVEN** user starts the local executable TUI outside a Git repository or without the Git command
- **WHEN** startup validation runs
- **THEN** startup fails with an actionable error before orchestration side effects
- **AND** the TUI does not offer a serial fallback
