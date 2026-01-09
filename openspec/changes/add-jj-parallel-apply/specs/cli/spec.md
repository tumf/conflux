## ADDED Requirements

### Requirement: Parallel Execution Mode Flag

The CLI SHALL support a `--parallel` flag to enable parallel change execution using jj workspaces.

#### Scenario: Enable parallel mode via CLI flag
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **THEN** the orchestrator enters parallel execution mode
- **AND** changes are analyzed for parallelization opportunities

#### Scenario: Parallel mode requires jj
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **AND** jj is not installed or not detected
- **THEN** the command exits with an error
- **AND** an error message indicates jj is required for parallel mode

#### Scenario: Parallel mode with max concurrent limit
- **WHEN** user runs `openspec-orchestrator run --parallel --max-concurrent 4`
- **THEN** at most 4 workspaces are created simultaneously
- **AND** additional changes wait until a workspace becomes available

### Requirement: Parallel Mode Conflict Strategy Flag

The CLI SHALL support a `--conflict-strategy` flag to specify how merge conflicts are handled.

#### Scenario: Fail on conflict (default)
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **AND** no `--conflict-strategy` is specified
- **THEN** the orchestrator stops on first merge conflict
- **AND** workspace state is preserved for inspection

#### Scenario: Skip conflicting changes
- **WHEN** user runs `openspec-orchestrator run --parallel --conflict-strategy skip`
- **AND** a merge conflict occurs
- **THEN** the conflicting change is skipped
- **AND** processing continues with remaining changes

#### Scenario: Resolve conflicts with AI
- **WHEN** user runs `openspec-orchestrator run --parallel --conflict-strategy resolve`
- **AND** a merge conflict occurs
- **THEN** the configured `resolve_command` is executed to resolve the conflict

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
