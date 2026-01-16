## ADDED Requirements

### Requirement: エージェントコマンドの設定ファイル

オーケストレーターは JSONC 形式の設定ファイルを通じてエージェントコマンドを設定できなければならない (MUST)。

設定可能なコマンドは以下の3種類とする:
1. `apply_command` - 変更の適用コマンド
2. `archive_command` - 変更のアーカイブコマンド
3. `analyze_command` - 依存関係分析コマンド

#### Scenario: プロジェクト設定ファイルが存在する場合

- **WHEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "apply_command": "codex run 'openspec-apply {change_id}'",
    "archive_command": "codex run 'conflux:archive {change_id}'"
  }
  ```
- **AND** 変更 `add-feature` を適用する
- **THEN** `codex run 'openspec-apply add-feature'` が実行される

#### Scenario: 設定ファイルが存在しない場合のフォールバック

- **WHEN** `.cflx.jsonc` が存在しない
- **AND** グローバル設定も存在しない
- **AND** 変更 `add-feature` を適用する
- **THEN** デフォルトの OpenCode コマンド `opencode run '/openspec-apply add-feature'` が実行される

#### Scenario: 部分的な設定のフォールバック

- **WHEN** `.cflx.jsonc` に `apply_command` のみ設定されている
- **AND** `archive_command` が設定されていない
- **THEN** `archive_command` はデフォルトの OpenCode コマンドが使用される

### Requirement: 設定ファイルの優先順位

オーケストレーターは以下の優先順位で設定ファイルを読み込まなければならない (MUST):

1. **プロジェクト設定** (優先): `.cflx.jsonc` (プロジェクトルート)
2. **グローバル設定**: `~/.config/cflx/config.jsonc`

プロジェクト設定が存在する場合はそちらを使用し、存在しない場合のみグローバル設定を使用する。

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

### Requirement: プレースホルダーの展開

コマンドテンプレート内のプレースホルダーは、実行時に実際の値に置換されなければならない (MUST)。

サポートするプレースホルダー:
- `{change_id}` - 変更ID（apply, archive コマンドで使用）
- `{prompt}` - 分析プロンプト（analyze コマンドで使用）

#### Scenario: {change_id} プレースホルダーの正常な展開

- **WHEN** `apply_command` が `"agent run --apply {change_id}"` と設定されている
- **AND** 変更ID が `update-auth` である
- **THEN** 実行されるコマンドは `agent run --apply update-auth` となる

#### Scenario: 複数の {change_id} プレースホルダー

- **WHEN** `apply_command` が `"agent --id {change_id} --name {change_id}"` と設定されている
- **AND** 変更ID が `fix-bug` である
- **THEN** 実行されるコマンドは `agent --id fix-bug --name fix-bug` となる

#### Scenario: {prompt} プレースホルダーの展開

- **WHEN** `analyze_command` が `"claude '{prompt}'"` と設定されている
- **AND** 分析プロンプトが `"次に実行すべき変更を選択してください"` である
- **THEN** 実行されるコマンドは `claude '次に実行すべき変更を選択してください'` となる

### Requirement: 依存関係分析コマンドの設定

`analyze_command` は LLM による依存関係分析に使用するコマンドを設定できなければならない (MUST)。このコマンドは `{prompt}` プレースホルダーを使用して分析プロンプトを受け取る。

#### Scenario: カスタム分析コマンドの使用

- **WHEN** `analyze_command` が `"claude-code '{prompt}'"` と設定されている
- **AND** 依存関係分析が実行される
- **THEN** `claude-code` にプロンプトが渡され、その出力が解析される

#### Scenario: 分析コマンド未設定時のフォールバック

- **WHEN** `analyze_command` が設定されていない
- **THEN** デフォルトの `opencode run --format json '{prompt}'` が使用される

### Requirement: JSONC 形式のサポート

設定ファイルは JSON with Comments (JSONC) 形式をサポートしなければならない (MUST)。

#### Scenario: コメント付き設定ファイルの解析

- **WHEN** 設定ファイルに以下の内容が含まれる:
  ```jsonc
  {
    // 適用コマンドの設定
    "apply_command": "codex run 'openspec-apply {change_id}'"
  }
  ```
- **THEN** コメントは無視され、設定が正常に読み込まれる

#### Scenario: 末尾カンマの許容

- **WHEN** 設定ファイルに末尾カンマが含まれる:
  ```jsonc
  {
    "apply_command": "codex run '{change_id}'",
  }
  ```
- **THEN** 末尾カンマは無視され、設定が正常に読み込まれる
