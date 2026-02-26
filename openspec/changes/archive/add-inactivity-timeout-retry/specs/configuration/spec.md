## MODIFIED Requirements

### Requirement: 無出力タイムアウト設定

オーケストレーターは JSONC 設定ファイルで無出力タイムアウトを設定できなければならない (MUST)。

設定可能な項目は以下の通りとする：

- `command_inactivity_timeout_secs` - 無出力が続いた場合に中断するまでの秒数（デフォルト: 900）
- `command_inactivity_kill_grace_secs` - タイムアウト検知後の終了猶予秒数（デフォルト: 5）
- `command_inactivity_timeout_max_retries` - 無出力タイムアウトで中断した場合のリトライ回数（デフォルト: 0）

#### Scenario: デフォルト設定で無出力タイムアウトが有効
- **WHEN** 設定ファイルに無出力タイムアウト設定が存在しない
- **THEN** `command_inactivity_timeout_secs` は 900 秒として扱われる
- **AND** `command_inactivity_kill_grace_secs` は 5 秒として扱われる
- **AND** `command_inactivity_timeout_max_retries` は 0 として扱われる

#### Scenario: 無出力タイムアウトの無効化

- **GIVEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_inactivity_timeout_secs": 0
  }
  ```
- **WHEN** コマンドが長時間無出力で実行される
- **THEN** 無出力タイムアウトは適用されない

#### Scenario: 無出力タイムアウトのリトライ

- **GIVEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_inactivity_timeout_secs": 900,
    "command_inactivity_timeout_max_retries": 3,
    "command_queue_retry_delay_ms": 5000
  }
  ```
- **WHEN** コマンドが無出力タイムアウトで中断される
- **THEN** コマンドは最大 3 回まで自動リトライされる
- **AND** 各リトライの間に `command_queue_retry_delay_ms` の待機が発生する
