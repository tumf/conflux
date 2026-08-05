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

### Requirement: run Subcommand

The `run` subcommand SHALL execute explicit targets through cumulative worktree orchestration. Push post-archive mode and upstream integration remain mutually exclusive; upstream integration SHALL be rejected for unsupported remote-client or server surfaces before work starts.

#### Scenario: run uses explicit targets

- **WHEN** user provides `--all`, positional change IDs, or legacy `--change` values
- **THEN** targets are normalized and dispatched through worktree orchestration
- **AND** no execution-mode selection is required

### Requirement: Apply Context History

The orchestrator MUST use one history-injection loop for every managed-worktree apply attempt. Each attempt summary and one-shot acceptance failure tail MUST retain their existing bounded injection semantics.

#### Scenario: second apply includes history

- **GIVEN** a change's first managed-worktree apply returned an agent summary
- **WHEN** its second apply starts
- **THEN** the prompt includes `<last_apply attempt="1">`
- **AND** the block contains the first summary

### Requirement: Git Repository Detection

Executable CLI orchestration SHALL require a usable Git repository and Git command. Validation SHALL happen before orchestration side effects.

#### Scenario: Git repository unavailable

- **WHEN** user starts `cflx run --all` outside a usable Git repository
- **THEN** the command exits non-zero with an actionable Git error
- **AND** no hook, lifecycle adapter, AI subprocess, or workspace mutation starts

### Requirement: Enhanced Help Output

CLI help SHALL document all current subcommands and supported options. It SHALL include `--max-concurrent`, `--dry-run`, `--vcs`, web controls, and `--push [remote]` where accepted, and SHALL NOT advertise `--parallel`.

#### Scenario: Run help describes standard worktree execution

- **WHEN** user runs `cflx run --help`
- **THEN** help describes explicit targets, concurrency, dry-run, VCS, web monitoring, and post-archive options
- **AND** it contains no `--parallel` option or mode-selection example

### Requirement: run Surfaces Hook Output

The `run` subcommand SHALL preserve hook command, output, and failure ordering on the sole worktree execution path.

#### Scenario: CLI run preserves hook visibility

- **GIVEN** a hook is configured for a lifecycle stage reached during `cflx run`
- **WHEN** the hook executes
- **THEN** users see the hook command followed by captured output and any failure result
- **AND** debug-only tracing is not required

## REMOVED Requirements

### Requirement: Serial Apply Iteration WIP Commits

Removed because repository-root serial apply execution no longer exists. Progress commits required by managed-worktree apply execution remain governed by the common apply and worktree execution requirements.

### Requirement: Serial run resolves workflow state from its captured repository root

Removed because no serial run service remains. The sole worktree orchestration path continues to derive authoritative workflow state from repository and workspace evidence under the Constitution.
