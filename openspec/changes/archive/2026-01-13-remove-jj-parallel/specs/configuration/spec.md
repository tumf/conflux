## MODIFIED Requirements
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
- **AND** git repository is required (`.git` directory must exist)

#### Scenario: Configure max concurrent workspaces
- **WHEN** config file contains `"max_concurrent_workspaces": 5`
- **THEN** at most 5 workspaces are created simultaneously
- **AND** CLI `--max-concurrent` overrides this value if provided

#### Scenario: Default max concurrent value
- **WHEN** `max_concurrent_workspaces` is not specified
- **THEN** the default value is 3

### Requirement: Workspace Base Directory Configuration

The orchestrator SHALL support configuring the base directory for git worktrees.

#### Scenario: Configure workspace directory
- **WHEN** config file contains `"workspace_base_dir": "/var/tmp/openspec-ws"`
- **THEN** worktrees are created under `/var/tmp/openspec-ws/`

#### Scenario: Default workspace directory
- **WHEN** `workspace_base_dir` is not specified
- **THEN** worktrees are created under system temp directory (e.g., `/tmp/openspec-workspaces/`)

### Requirement: Parallel Configuration in Templates

The `init` command templates SHALL include parallel execution configuration options.

#### Scenario: Claude template with parallel options
- **WHEN** user runs `openspec-orchestrator init --template claude`
- **THEN** the generated config includes commented parallel configuration:
  ```jsonc
  {
    // Parallel execution (requires git worktree)
    // "parallel_mode": false,
    // "max_concurrent_workspaces": 3
  }
  ```

### Requirement: VCS Backend Configuration

設定ファイルで VCS バックエンドを指定できなければならない（SHALL）。

#### Scenario: Configure VCS backend in config file

- **WHEN** `.openspec-orchestrator.jsonc` に以下が設定されている:
  ```jsonc
  {
    "vcs_backend": "git"
  }
  ```
- **AND** `--parallel` フラグで実行される
- **THEN** Git バックエンドが使用される

#### Scenario: VCS backend values

- **WHEN** `vcs_backend` を設定する
- **THEN** 有効な値は `"auto"`, `"git"` である
- **AND** デフォルト値は `"auto"` である

#### Scenario: CLI flag overrides config

- **WHEN** config ファイルに `"vcs_backend": "auto"` が設定されている
- **AND** `--vcs git` フラグが指定される
- **THEN** Git バックエンドが使用される（CLI が優先）

#### Scenario: Invalid VCS backend in config

- **WHEN** config ファイルに `"vcs_backend": "invalid"` が設定されている
- **THEN** 設定読み込み時にエラーが発生する
- **AND** エラーメッセージに有効な値が表示される

### Requirement: VCS Configuration in Templates

`init` コマンドで生成されるテンプレートに VCS 設定オプションを含めなければならない（SHALL）。

#### Scenario: Template includes VCS configuration

- **WHEN** `openspec-orchestrator init` が実行される
- **THEN** 生成される設定ファイルに以下のコメント付き設定が含まれる:
  ```jsonc
  {
    // VCS backend for parallel execution
    // "auto": detect automatically (git only)
    // "git": use git worktree
    // "vcs_backend": "auto"
  }
  ```

## REMOVED Requirements
### Requirement: Automatic Conflict Resolution
**Reason**: jj コマンド前提の自動解決が不要になるため。
**Migration**: Git コンフリクト解決の仕様に統一する。
