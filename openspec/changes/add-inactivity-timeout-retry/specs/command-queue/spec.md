## MODIFIED Requirements

### Requirement: 無出力タイムアウトによる中断

コマンドキューは streaming 実行中に stdout/stderr の出力が一定時間発生しない場合、無出力タイムアウトとしてコマンドを中断しなければならない (MUST)。

無出力タイムアウトの動作は以下の通りとする：

- 出力行（stdout/stderr）の受信時刻を記録する
- 設定された無出力タイムアウト秒数を超えた場合、コマンドを終了させる
- 終了時は警告ログを出力し、エラーメッセージに「inactivity timeout」を含める
- 強制終了は猶予時間を設け、猶予内に終了しない場合は強制 kill する

加えて、設定 `command_inactivity_timeout_max_retries` が 1 以上の場合、無出力タイムアウトで中断されたコマンドは自動的に再実行されなければならない (MUST)。

- リトライ回数は `command_inactivity_timeout_max_retries` に従う
- リトライ待機は `command_queue_retry_delay_ms` に従う
- リトライ理由はユーザーに分かる形で streaming 出力（stderr）に通知されなければならない (MUST)

#### Scenario: 無出力タイムアウトで中断した場合に 3 回リトライ

- **GIVEN** `command_inactivity_timeout_secs` が 900 秒に設定されている
- **AND** `command_inactivity_timeout_max_retries` が 3 に設定されている
- **WHEN** コマンドが無出力タイムアウトで中断される
- **THEN** コマンドは最大 3 回まで自動リトライされる
- **AND** streaming 出力に `inactivity timeout` を理由とするリトライ通知が含まれる
