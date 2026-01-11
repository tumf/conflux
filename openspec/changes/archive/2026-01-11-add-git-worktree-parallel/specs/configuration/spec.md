## ADDED Requirements

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
- **THEN** 有効な値は `"auto"`, `"jj"`, `"git"` である
- **AND** デフォルト値は `"auto"` である

#### Scenario: CLI flag overrides config

- **WHEN** config ファイルに `"vcs_backend": "git"` が設定されている
- **AND** `--vcs jj` フラグが指定される
- **THEN** jj バックエンドが使用される（CLI が優先）

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
    // "auto": detect automatically (jj preferred, then git)
    // "jj": use jj workspaces
    // "git": use git worktree
    // "vcs_backend": "auto"
  }
  ```
