## ADDED Requirements

### Requirement: jj Repository Detection

The CLI SHALL detect whether the current directory is a jj-managed repository by checking for the `.jj` directory.

#### Scenario: jj repository detected
- **WHEN** a `.jj` directory exists in the current working directory
- **THEN** jj features (parallel mode) are available

#### Scenario: jj repository not detected
- **WHEN** no `.jj` directory exists in the current working directory
- **AND** user runs `openspec-orchestrator run --parallel`
- **THEN** the command exits with a non-zero exit code
- **AND** an error message is displayed: "Error: --parallel requires a jj repository (.jj directory not found)"

### Requirement: Parallel Execution Mode Flag

The CLI SHALL support a `--parallel` flag to enable parallel change execution using jj workspaces. Parallel mode is OFF by default.

#### Scenario: Enable parallel mode via CLI flag
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **AND** a `.jj` directory exists
- **THEN** the orchestrator enters parallel execution mode
- **AND** changes are analyzed for parallelization opportunities

#### Scenario: Parallel mode disabled by default
- **WHEN** user runs `openspec-orchestrator run` without `--parallel` flag
- **THEN** the orchestrator uses sequential execution mode
- **AND** no parallelization analysis is performed

#### Scenario: Parallel mode requires jj directory
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **AND** no `.jj` directory exists
- **THEN** the command exits with error code 1
- **AND** an error message indicates jj repository is required for parallel mode

#### Scenario: Parallel mode with max concurrent limit
- **WHEN** user runs `openspec-orchestrator run --parallel --max-concurrent 4`
- **THEN** at most 4 workspaces are created simultaneously
- **AND** additional changes wait until a workspace becomes available

### Requirement: Parallel Mode TUI Display

The TUI SHALL display parallel execution progress when in parallel mode.

#### Scenario: Display parallel groups
- **WHEN** TUI is in running mode with parallel execution
- **THEN** changes are grouped by their parallel group assignment
- **AND** each group is visually distinguished

#### Scenario: Display workspace status
- **WHEN** changes are being processed in parallel
- **THEN** each change shows its workspace status (creating, running, completed, failed)
- **AND** multiple spinners can be active simultaneously

#### Scenario: Display merge progress
- **WHEN** a parallel group completes and merge begins
- **THEN** a merge progress indicator is displayed
- **AND** the merge result (success/conflict) is shown

### Requirement: Parallel Mode Dry Run

The CLI SHALL support `--dry-run` to preview parallelization groups without execution.

#### Scenario: Preview parallelization groups
- **WHEN** user runs `openspec-orchestrator run --parallel --dry-run`
- **THEN** the analyzer determines parallelization groups
- **AND** the groups are displayed without executing any changes
- **AND** no workspaces are created
