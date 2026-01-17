## MODIFIED Requirements

### Requirement: Workspace Base Directory Configuration
The orchestrator SHALL support configuring the base directory for git worktrees.

#### Scenario: Configure workspace directory
- **WHEN** config file contains "workspace_base_dir": "/var/tmp/openspec-ws"
- **THEN** worktrees are created under `/var/tmp/openspec-ws/`

#### Scenario: Default workspace directory
- **GIVEN** `workspace_base_dir` is not specified
- **WHEN** the orchestrator resolves the default workspace directory
- **THEN** macOS uses `${XDG_DATA_HOME}/conflux/worktrees/<project_slug>` when `XDG_DATA_HOME` is set
- **AND** macOS falls back to `~/Library/Application Support/conflux/worktrees/<project_slug>` when `XDG_DATA_HOME` is not set
- **AND** Linux uses `${XDG_DATA_HOME:-~/.local/share}/conflux/worktrees/<project_slug>`
- **AND** Windows uses `%APPDATA%\Conflux\worktrees\<project_slug>`
- **AND** `<project_slug>` is derived from the repository name plus a short hash of the absolute repository path
