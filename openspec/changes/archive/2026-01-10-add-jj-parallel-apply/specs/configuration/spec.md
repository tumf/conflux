## ADDED Requirements

### Requirement: Parallel Execution Configuration

The orchestrator SHALL support parallel execution configuration options in the config file. Parallel mode is OFF by default.

#### Scenario: Parallel mode disabled by default
- **WHEN** config file does not contain `"parallel_mode"` key
- **THEN** parallel execution mode is disabled
- **AND** CLI `--parallel` flag is required to enable it

#### Scenario: Enable parallel mode via config
- **WHEN** config file contains `"parallel_mode": true`
- **THEN** parallel execution mode is enabled by default
- **AND** CLI `--parallel` flag is not required
- **AND** jj repository is still required (`.jj` directory must exist)

#### Scenario: Configure max concurrent workspaces
- **WHEN** config file contains `"max_concurrent_workspaces": 5`
- **THEN** at most 5 workspaces are created simultaneously
- **AND** CLI `--max-concurrent` overrides this value if provided

#### Scenario: Default max concurrent value
- **WHEN** `max_concurrent_workspaces` is not specified
- **THEN** the default value is 3

### Requirement: Workspace Base Directory Configuration

The orchestrator SHALL support configuring the base directory for jj workspaces.

#### Scenario: Configure workspace directory
- **WHEN** config file contains `"workspace_base_dir": "/var/tmp/openspec-ws"`
- **THEN** workspaces are created under `/var/tmp/openspec-ws/`

#### Scenario: Default workspace directory
- **WHEN** `workspace_base_dir` is not specified
- **THEN** workspaces are created under system temp directory (e.g., `/tmp/openspec-workspaces/`)

### Requirement: Automatic Conflict Resolution

The orchestrator SHALL automatically resolve merge conflicts using AI agent with jj commands. No manual configuration is required.

#### Scenario: Conflict detected after merge
- **WHEN** `jj new` creates a merge commit
- **AND** `jj status` indicates conflicts exist
- **THEN** the orchestrator invokes AI agent with hardcoded resolution prompt
- **AND** the prompt includes conflicted file list and jj commands

#### Scenario: Hardcoded resolution prompt
- **WHEN** conflicts are detected
- **THEN** the following prompt is used (not configurable):
  ```
  The merge resulted in conflicts. Use jj commands to resolve them.

  Conflicted files:
  {conflict_files}

  Steps:
  1. Run `jj status` to see conflict details
  2. For each conflicted file, either:
     - Edit the file to resolve conflict markers, OR
     - Run `jj resolve <file>` to use merge tool
  3. After resolving, run `jj status` to verify no conflicts remain
  ```

#### Scenario: Resolution success
- **WHEN** AI agent resolves conflicts
- **AND** `jj status` shows no conflicts
- **THEN** processing continues with next group

#### Scenario: Resolution failure after retries
- **WHEN** AI agent cannot resolve conflicts
- **AND** max retries (default: 3) exceeded
- **THEN** orchestrator stops with error
- **AND** workspace state is preserved for manual inspection
- **AND** error message includes workspace path and `jj status` output

### Requirement: Parallelization Analysis Prompt Configuration

The orchestrator SHALL support customizing the parallelization analysis prompt.

#### Scenario: Custom parallelization prompt
- **WHEN** config file contains `"parallelization_prompt": "custom prompt {changes}"`
- **THEN** the custom prompt is used for parallelization analysis
- **AND** `{changes}` is replaced with the list of pending changes

#### Scenario: Default parallelization prompt
- **WHEN** `parallelization_prompt` is not configured
- **THEN** a default prompt is used that asks the LLM to identify independent changes

### Requirement: Analyzer Dependency Output

The parallelization analyzer MUST return dependency information between changes to enable correct execution ordering.

#### Scenario: Analyzer returns dependency groups
- **WHEN** parallelization analysis is performed
- **THEN** the analyzer returns JSON with groups containing `depends_on` field:
  ```json
  {
    "groups": [
      {"id": 1, "changes": ["feature-a", "feature-b"], "depends_on": []},
      {"id": 2, "changes": ["integrate-ab"], "depends_on": [1]}
    ]
  }
  ```
- **AND** changes within the same group can run in parallel
- **AND** groups with `depends_on` wait for dependent groups to complete

#### Scenario: Circular dependency detection
- **WHEN** analyzer detects circular dependencies between changes
- **THEN** an error is returned with details about the circular dependency
- **AND** parallel execution is aborted

#### Scenario: Single change has no dependencies
- **WHEN** a change has no dependencies on other changes
- **THEN** the change is placed in a group with `depends_on: []`
- **AND** can run in parallel with other independent changes

#### Scenario: All changes are sequential
- **WHEN** analyzer determines all changes have dependencies
- **THEN** each change is placed in its own group
- **AND** `depends_on` forms a chain of sequential execution

### Requirement: Parallel Configuration in Templates

The `init` command templates SHALL include parallel execution configuration options.

#### Scenario: Claude template with parallel options
- **WHEN** user runs `openspec-orchestrator init --template claude`
- **THEN** the generated config includes commented parallel configuration:
  ```jsonc
  {
    // Parallel execution (requires jj)
    // "parallel_mode": false,
    // "max_concurrent_workspaces": 3
  }
  ```
