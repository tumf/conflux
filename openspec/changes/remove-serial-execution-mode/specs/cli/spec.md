## MODIFIED Requirements

### Requirement: Parallel Execution Mode Flag

The CLI SHALL use cumulative Git-worktree orchestration for every executable `run` invocation. Execution mode SHALL NOT be selectable through a `--parallel` or serial-equivalent flag.

#### Scenario: Default run uses worktree orchestration

- **WHEN** user runs `cflx run --all` in a usable Git repository
- **THEN** the orchestrator analyzes eligible changes and dispatches cumulative worktree execution
- **AND** no execution-mode flag is required

#### Scenario: Single change uses the same execution path

- **WHEN** user runs `cflx run my-feature` in a usable Git repository
- **THEN** `my-feature` executes in a managed worktree
- **AND** it follows the same archive and post-archive state transitions as a multi-change run

#### Scenario: Retired parallel flag is rejected

- **WHEN** user runs `cflx run --parallel`
- **THEN** argument parsing fails with a non-zero exit status
- **AND** help output does not advertise `--parallel`

#### Scenario: Execution requires Git before side effects

- **WHEN** user starts executable run orchestration outside a usable Git repository or without the Git command
- **THEN** startup fails with a non-zero exit status and an actionable error
- **AND** no hook, lifecycle adapter, AI subprocess, or managed-worktree mutation has started

#### Scenario: Concurrency remains configurable

- **WHEN** user runs `cflx run --all --max-concurrent 4`
- **THEN** at most 4 managed workspaces execute simultaneously
- **AND** additional eligible changes wait until capacity is available

### Requirement: Parallel Mode TUI Display

The TUI SHALL display worktree execution, workspace, and post-archive progress as the standard execution presentation. It SHALL NOT display a selectable execution-mode badge or mode toggle.

#### Scenario: Display concurrent worktree progress

- **WHEN** TUI is running one or more changes
- **THEN** each change shows its managed-workspace status
- **AND** concurrently active changes may show simultaneous progress

#### Scenario: Display post-archive progress

- **WHEN** an archived change enters merge, resolve, or push handling
- **THEN** the corresponding progress and terminal result are displayed
- **AND** archive alone is not presented as terminal because of an execution mode

### Requirement: Parallel Mode Dry Run

The CLI SHALL support `--dry-run` to preview dependency groups without execution on the standard worktree orchestration path.

#### Scenario: Preview dependency groups

- **WHEN** user runs `cflx run --all --dry-run`
- **THEN** the analyzer determines execution groups
- **AND** the groups are displayed without executing changes
- **AND** no managed workspace is created or mutated

### Requirement: VCS Backend Selection Flag

CLI SHALL apply `--vcs` directly to the sole worktree execution path.

#### Scenario: Default auto detection

- **WHEN** an executable run starts without `--vcs`
- **THEN** the VCS backend is auto-detected
- **AND** Git is selected in a usable Git repository

#### Scenario: Explicit git selection

- **WHEN** `cflx run --all --vcs git` is executed
- **THEN** the Git backend is used
- **AND** startup fails before orchestration side effects if Git is unavailable

#### Scenario: Explicit auto selection

- **WHEN** `cflx run --all --vcs auto` is executed
- **THEN** the VCS backend is auto-detected
- **AND** Git is selected in a usable Git repository

#### Scenario: Invalid VCS value

- **WHEN** `cflx run --all --vcs invalid` is executed
- **THEN** error message "Invalid VCS backend: invalid. Valid options: auto, git" is displayed
- **AND** exit code is non-zero

## REMOVED Requirements

### Requirement: Serial Apply Iteration WIP Commits

Removed because repository-root serial apply execution no longer exists. Progress commits required by managed-worktree apply execution remain governed by the common apply and worktree execution requirements.

### Requirement: Serial run resolves workflow state from its captured repository root

Removed because no serial run service remains. The sole worktree orchestration path continues to derive authoritative workflow state from repository and workspace evidence under the Constitution.
