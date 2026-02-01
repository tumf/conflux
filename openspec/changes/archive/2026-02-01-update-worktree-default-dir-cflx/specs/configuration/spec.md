## MODIFIED Requirements

### Requirement: Workspace Base Directory Configuration
オーケストレーターは git worktree のベースディレクトリを設定できなければならない (MUST)。

#### Scenario: Configure workspace directory
- **WHEN** config file contains "workspace_base_dir": "/var/tmp/openspec-ws"
- **THEN** worktrees are created under `/var/tmp/openspec-ws/`

#### Scenario: Default workspace directory
- **GIVEN** `workspace_base_dir` is not specified
- **WHEN** the orchestrator resolves the default workspace directory
- **THEN** macOS uses `${XDG_DATA_HOME}/cflx/worktrees/<project_slug>` when `XDG_DATA_HOME` is set
- **AND** macOS falls back to `~/.local/share/cflx/worktrees/<project_slug>` when `XDG_DATA_HOME` is not set
- **AND** Linux uses `${XDG_DATA_HOME:-~/.local/share}/cflx/worktrees/<project_slug>`
- **AND** Windows uses `%APPDATA%/cflx/worktrees/<project_slug>`
- **AND** `<project_slug>` is derived from the repository name plus a short hash of the absolute repository path
