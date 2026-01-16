## ADDED Requirements

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

## MODIFIED Requirements

### Requirement: エージェントコマンドの設定ファイル

オーケストレーターは JSONC 形式の設定ファイルを通じてエージェントコマンドを設定できなければならない (MUST)。

設定可能なコマンドは以下の4種類とする:
1. `apply_command` - 変更の適用コマンド
2. `archive_command` - 変更のアーカイブコマンド
3. `analyze_command` - 依存関係分析コマンド
4. `hooks` - 段階フックコマンド

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
