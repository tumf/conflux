## ADDED Requirements

### Requirement: Parallel Execution Configuration

The orchestrator SHALL support parallel execution configuration options in the config file.

#### Scenario: Enable parallel mode via config
- **WHEN** config file contains `"parallel_mode": true`
- **THEN** parallel execution mode is enabled by default
- **AND** CLI `--parallel` flag is not required

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

### Requirement: Conflict Strategy Configuration

The orchestrator SHALL support configuring the conflict resolution strategy.

#### Scenario: Configure fail strategy
- **WHEN** config file contains `"conflict_strategy": "fail"`
- **THEN** orchestrator stops on first merge conflict

#### Scenario: Configure skip strategy
- **WHEN** config file contains `"conflict_strategy": "skip"`
- **THEN** conflicting changes are skipped and processing continues

#### Scenario: Configure resolve strategy
- **WHEN** config file contains `"conflict_strategy": "resolve"`
- **THEN** the `resolve_command` is used to resolve conflicts

#### Scenario: Default conflict strategy
- **WHEN** `conflict_strategy` is not specified
- **THEN** the default value is `"fail"`

### Requirement: Conflict Resolution Command

The orchestrator SHALL support a `resolve_command` for AI-assisted conflict resolution.

#### Scenario: Configure resolve command
- **WHEN** config file contains:
  ```jsonc
  {
    "resolve_command": "claude --dangerously-skip-permissions -p 'Resolve the following merge conflict: {conflict_info}'"
  }
  ```
- **AND** a merge conflict occurs with `conflict_strategy: "resolve"`
- **THEN** the resolve command is executed with conflict information

#### Scenario: Resolve command placeholder expansion
- **WHEN** `resolve_command` contains `{conflict_info}`
- **AND** a conflict occurs between changes `add-feature-a` and `add-feature-b`
- **THEN** `{conflict_info}` is replaced with conflict details (files, conflicting changes)

#### Scenario: Resolve command not configured
- **WHEN** `conflict_strategy` is `"resolve"`
- **AND** `resolve_command` is not configured
- **THEN** the orchestrator uses the default `analyze_command` with a conflict resolution prompt

### Requirement: Parallelization Analysis Prompt Configuration

The orchestrator SHALL support customizing the parallelization analysis prompt.

#### Scenario: Custom parallelization prompt
- **WHEN** config file contains `"parallelization_prompt": "custom prompt {changes}"`
- **THEN** the custom prompt is used for parallelization analysis
- **AND** `{changes}` is replaced with the list of pending changes

#### Scenario: Default parallelization prompt
- **WHEN** `parallelization_prompt` is not configured
- **THEN** a default prompt is used that asks the LLM to identify independent changes

### Requirement: Parallel Configuration in Templates

The `init` command templates SHALL include parallel execution configuration options.

#### Scenario: Claude template with parallel options
- **WHEN** user runs `openspec-orchestrator init --template claude`
- **THEN** the generated config includes commented parallel configuration:
  ```jsonc
  {
    // Parallel execution (requires jj)
    // "parallel_mode": false,
    // "max_concurrent_workspaces": 3,
    // "conflict_strategy": "fail",
    // "resolve_command": "claude -p 'Resolve conflict: {conflict_info}'"
  }
  ```
