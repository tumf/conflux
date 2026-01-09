# configuration Specification

## Purpose
TBD - created by archiving change add-env-openspec-cmd. Update Purpose after archive.
## Requirements
### Requirement: Environment Variable Configuration for OpenSpec Command

ユーザーは環境変数 `OPENSPEC_CMD` を通じて openspec コマンドを設定できなければならない (MUST)。

設定値の優先順位は以下の通りとする:
1. CLI 引数 `--openspec-cmd` (最優先)
2. 環境変数 `OPENSPEC_CMD`
3. デフォルト値 `npx @fission-ai/openspec@latest`

#### Scenario: 環境変数のみ設定

- **WHEN** 環境変数 `OPENSPEC_CMD` に `/usr/local/bin/openspec` が設定されている
- **AND** CLI 引数 `--openspec-cmd` が指定されていない
- **THEN** `/usr/local/bin/openspec` が openspec コマンドとして使用される

#### Scenario: CLI 引数が環境変数より優先

- **WHEN** 環境変数 `OPENSPEC_CMD` に `/usr/local/bin/openspec` が設定されている
- **AND** CLI 引数 `--openspec-cmd ./my-openspec` が指定されている
- **THEN** `./my-openspec` が openspec コマンドとして使用される

#### Scenario: どちらも未設定時はデフォルト値を使用

- **WHEN** 環境変数 `OPENSPEC_CMD` が設定されていない
- **AND** CLI 引数 `--openspec-cmd` が指定されていない
- **THEN** `npx @fission-ai/openspec@latest` が openspec コマンドとして使用される

### Requirement: エージェントコマンドの設定ファイル

オーケストレーターは JSONC 形式の設定ファイルを通じてエージェントコマンドを設定できなければならない (MUST)。

設定可能なコマンドは以下の4種類とする:
1. `apply_command` - 変更の適用コマンド
2. `archive_command` - 変更のアーカイブコマンド
3. `analyze_command` - 依存関係分析コマンド
4. `hooks` - 段階フックコマンド

#### Scenario: プロジェクト設定ファイルで hooks を設定

- **WHEN** `.openspec-orchestrator.jsonc` に以下の設定が存在する:
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

### Requirement: 設定ファイルの優先順位

オーケストレーターは以下の優先順位で設定ファイルを読み込まなければならない (MUST):

1. **プロジェクト設定** (優先): `.openspec-orchestrator.jsonc` (プロジェクトルート)
2. **グローバル設定**: `~/.config/openspec-orchestrator/config.jsonc`

プロジェクト設定が存在する場合はそちらを使用し、存在しない場合のみグローバル設定を使用する。

#### Scenario: プロジェクト設定がグローバル設定より優先される

- **GIVEN** グローバル設定 `~/.config/openspec-orchestrator/config.jsonc` に:
  ```jsonc
  { "apply_command": "global-agent apply {change_id}" }
  ```
- **AND** プロジェクト設定 `.openspec-orchestrator.jsonc` に:
  ```jsonc
  { "apply_command": "project-agent apply {change_id}" }
  ```
- **WHEN** 変更 `fix-bug` を適用する
- **THEN** `project-agent apply fix-bug` が実行される（プロジェクト設定が優先）

#### Scenario: プロジェクト設定がない場合はグローバル設定を使用

- **GIVEN** プロジェクトルートに `.openspec-orchestrator.jsonc` が存在しない
- **AND** グローバル設定 `~/.config/openspec-orchestrator/config.jsonc` に:
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

### Requirement: フックコマンドの設定

オーケストレータは設定ファイルに `hooks` セクションを持ち、各段階に対応した任意コマンドを定義できなければならない (MUST)。

フックはすべてオプションであり、未設定のフックは実行されない。

#### Scenario: hooks 未設定

- **GIVEN** 設定ファイルに `hooks` セクションが存在しない
- **WHEN** オーケストレータを実行する
- **THEN** フックコマンドは一切実行されない

#### Scenario: 文字列（短縮形）でフックを設定

- **GIVEN** 設定ファイルに以下が存在する:
  ```jsonc
  {
    "hooks": {
      "on_start": "echo 'started'"
    }
  }
  ```
- **WHEN** オーケストレータを実行する
- **THEN** 開始時に `echo 'started'` が実行される

### Requirement: フック設定の詳細オプション

オーケストレータはフックごとに `continue_on_failure` と `timeout` を設定できなければならない (MUST)。

- `continue_on_failure` のデフォルト値は `true` とする
- `timeout` のデフォルト値は 60 秒とする

#### Scenario: continue_on_failure=false の場合はフック失敗で停止

- **GIVEN** `hooks.post_apply` が以下のように設定されている:
  ```jsonc
  {
    "hooks": {
      "post_apply": {
        "command": "exit 1",
        "continue_on_failure": false,
        "timeout": 60
      }
    }
  }
  ```
- **WHEN** post_apply が実行される
- **THEN** オーケストレータはエラーとして扱い処理を中断する

#### Scenario: timeout の超過

- **GIVEN** `hooks.on_start.timeout` が 1 秒に設定されている
- **AND** `hooks.on_start.command` がタイムアウトを超えて実行される
- **WHEN** `on_start` が実行される
- **THEN** フックはタイムアウトとして失敗扱いになる

### Requirement: フックのコンテキスト（プレースホルダーと環境変数）

オーケストレータはフック実行時に、コマンド文字列内のプレースホルダーを展開し、同等の情報を環境変数としても提供しなければならない (MUST)。

#### Scenario: change_id をプレースホルダーと環境変数で受け取る

- **GIVEN** `hooks.pre_apply.command` が `echo '{change_id} $OPENSPEC_CHANGE_ID'` に設定されている
- **WHEN** change `add-feature-x` に対して `pre_apply` が実行される
- **THEN** `{change_id}` は `add-feature-x` に展開される
- **AND** `OPENSPEC_CHANGE_ID` は `add-feature-x` として渡される

