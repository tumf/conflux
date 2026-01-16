## MODIFIED Requirements

### Requirement: エージェントコマンドの設定ファイル

オーケストレーターは JSONC 形式の設定ファイルを通じてエージェントコマンドを設定できなければならない (MUST)。

設定可能なコマンドは以下の5種類とする:
1. `apply_command` - 変更の適用コマンド
2. `archive_command` - 変更のアーカイブコマンド
3. `analyze_command` - 依存関係分析コマンド
4. `resolve_command` - Git マージの完了（merge/add/commit）や競合解消に使用するコマンド
5. `hooks` - 段階フックコマンド

#### Scenario: プロジェクト設定ファイルで hooks を設定

- **WHEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "hooks": {
      "on_start": "echo 'start'",
      "on_finish": "echo 'finish {status}'"
    }
  }
  ```
- **AND** オーケストレータを実行する
- **THEN** 開始時に `echo 'start'` が実行される
- **AND** 終了時に `echo 'finish {status}'`（プレースホルダー展開後）が実行される

### Requirement: プレースホルダーの展開

コマンドテンプレート内のプレースホルダーは、実行時に実際の値に置換されなければならない (MUST)。

サポートするプレースホルダー:
- `{change_id}` - 変更ID（apply_command, archive_command で使用）
- `{prompt}` - システム提供の指示（apply_command, archive_command, analyze_command, resolve_command で使用）

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

#### Scenario: Both placeholders in apply command

- **WHEN** `apply_command` is `"agent --id {change_id} --instructions '{prompt}'"`
- **AND** change ID is `fix-bug`
- **AND** apply prompt is `"Focus on core changes"`
- **THEN** the executed command is `agent --id fix-bug --instructions 'Focus on core changes'`

#### Scenario: Multiple {prompt} placeholders

- **WHEN** `apply_command` is `"agent apply {change_id} --pre '{prompt}' --post '{prompt}'"`
- **AND** change ID is `fix-bug`
- **AND** apply prompt is `"Be careful"`
- **THEN** the executed command is `agent apply fix-bug --pre 'Be careful' --post 'Be careful'`
