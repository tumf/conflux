## RENAMED Requirements
- FROM: `### Requirement: jj Repository Detection`
- TO: `### Requirement: Git Repository Detection`

## MODIFIED Requirements
### Requirement: Git Repository Detection

The CLI SHALL detect whether the current directory is a git-managed repository by checking for the `.git` directory.

#### Scenario: git repository detected
- **WHEN** a `.git` directory exists in the current working directory
- **THEN** git worktree parallel features are available

#### Scenario: git repository not detected
- **WHEN** no `.git` directory exists in the current working directory
- **AND** user runs `openspec-orchestrator run --parallel`
- **THEN** the command exits with a non-zero exit code
- **AND** an error message is displayed: "Error: --parallel requires a git repository (.git directory not found)"

### Requirement: Parallel Execution Mode Flag

The CLI SHALL support a `--parallel` flag to enable parallel change execution using git worktrees. Parallel mode is OFF by default.

#### Scenario: Enable parallel mode via CLI flag
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **AND** a `.git` directory exists
- **THEN** the orchestrator enters parallel execution mode
- **AND** changes are analyzed for parallelization opportunities

#### Scenario: Parallel mode disabled by default
- **WHEN** user runs `openspec-orchestrator run` without `--parallel` flag
- **THEN** the orchestrator uses sequential execution mode
- **AND** no parallelization analysis is performed

#### Scenario: Parallel mode requires git directory
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **AND** no `.git` directory exists
- **THEN** the command exits with error code 1
- **AND** an error message indicates git repository is required for parallel mode

#### Scenario: Parallel mode with max concurrent limit
- **WHEN** user runs `openspec-orchestrator run --parallel --max-concurrent 4`
- **THEN** at most 4 workspaces are created simultaneously
- **AND** additional changes wait until a workspace becomes available

### Requirement: VCS Backend Selection Flag

CLI SHALL allow explicit VCS backend selection via `--vcs` flag.

#### Scenario: Default auto detection

- **WHEN** `--parallel` flag is specified
- **AND** `--vcs` flag is not specified
- **THEN** VCS backend is auto-detected
- **AND** Git backend is selected when a `.git` directory exists

#### Scenario: Explicit git selection

- **WHEN** `openspec-orchestrator run --parallel --vcs git` is executed
- **THEN** Git backend is used
- **AND** an error is displayed if Git is not available

#### Scenario: Explicit auto selection

- **WHEN** `openspec-orchestrator run --parallel --vcs auto` is executed
- **THEN** VCS backend is auto-detected
- **AND** Git backend is selected when a `.git` directory exists

#### Scenario: Invalid VCS value

- **WHEN** `openspec-orchestrator run --parallel --vcs invalid` is executed
- **THEN** error message "Invalid VCS backend: invalid. Valid options: auto, git" is displayed
- **AND** exit code is non-zero

#### Scenario: --vcs without --parallel

- **WHEN** `openspec-orchestrator run --vcs git` is executed
- **AND** `--parallel` flag is not specified
- **THEN** `--vcs` option is ignored
- **AND** normal sequential execution proceeds
