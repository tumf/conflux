## MODIFIED Requirements

### Requirement: 設定ファイルの優先順位

オーケストレーターは以下の優先順位で設定ファイルを読み込まなければならない (MUST):

1. **プロジェクト設定** (優先): `.cflx.jsonc` (プロジェクトルート)
2. **グローバル設定**: `~/.config/cflx/config.jsonc`

プロジェクト設定が存在する場合はそちらを使用し、存在しない場合のみグローバル設定を使用する。

**BREAKING**: 旧設定ファイル名 (`.openspec-orchestrator.jsonc`, `~/.config/openspec-orchestrator/`) は読み込まれない。

#### Scenario: プロジェクト設定がグローバル設定より優先される

- **GIVEN** グローバル設定 `~/.config/cflx/config.jsonc` に:
  ```jsonc
  { "apply_command": "global-agent apply {change_id}" }
  ```
- **AND** プロジェクト設定 `.cflx.jsonc` に:
  ```jsonc
  { "apply_command": "project-agent apply {change_id}" }
  ```
- **WHEN** 変更 `fix-bug` を適用する
- **THEN** `project-agent apply fix-bug` が実行される（プロジェクト設定が優先）

#### Scenario: プロジェクト設定がない場合はグローバル設定を使用

- **GIVEN** プロジェクトルートに `.cflx.jsonc` が存在しない
- **AND** グローバル設定 `~/.config/cflx/config.jsonc` に:
  ```jsonc
  { "apply_command": "global-agent apply {change_id}" }
  ```
- **WHEN** 変更 `fix-bug` を適用する
- **THEN** `global-agent apply fix-bug` が実行される

#### Scenario: 旧設定ファイル名は読み込まれない

- **GIVEN** `.openspec-orchestrator.jsonc` が存在する
- **AND** `.cflx.jsonc` が存在しない
- **WHEN** 設定を読み込む
- **THEN** デフォルト設定が使用される（旧ファイルは無視される）

### Requirement: worktree_command を設定できる

`worktree_command` でTUIの `+` キーから起動される worktree 上の提案作成コマンドを設定できなければならない (MUST)。

#### Scenario: worktree_command を設定できる

- **GIVEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "worktree_command": "opencode run --cwd {workspace_dir} '/openspec:proposal'"
  }
  ```
- **WHEN** ユーザーがTUIの `+` キーで提案作成フローを開始する
- **THEN** `worktree_command` が使用される
