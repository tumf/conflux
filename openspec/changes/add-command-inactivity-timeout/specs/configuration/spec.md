## ADDED Requirements
### Requirement: 無出力タイムアウト設定

オーケストレーターは JSONC 設定ファイルで無出力タイムアウトを設定できなければならない (MUST)。

設定可能な項目は以下の通りとする：
- `command_inactivity_timeout_secs` - 無出力が続いた場合に中断するまでの秒数（デフォルト: 900）
- `command_inactivity_kill_grace_secs` - タイムアウト検知後の終了猶予秒数（デフォルト: 5）

#### Scenario: デフォルト設定で無出力タイムアウトが有効
- **WHEN** 設定ファイルに無出力タイムアウト設定が存在しない
- **THEN** `command_inactivity_timeout_secs` は 900 秒として扱われる
- **AND** `command_inactivity_kill_grace_secs` は 5 秒として扱われる

#### Scenario: 無出力タイムアウトの無効化
- **GIVEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_inactivity_timeout_secs": 0
  }
  ```
- **WHEN** コマンドが長時間無出力で実行される
- **THEN** 無出力タイムアウトは適用されない

#### Scenario: カスタム猶予時間の設定
- **GIVEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_inactivity_timeout_secs": 120,
    "command_inactivity_kill_grace_secs": 10
  }
  ```
- **WHEN** コマンドが無出力タイムアウトに到達する
- **THEN** 終了猶予は 10 秒として扱われる
