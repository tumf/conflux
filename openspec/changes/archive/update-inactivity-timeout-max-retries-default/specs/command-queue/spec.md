## MODIFIED Requirements

### Requirement: 無出力タイムアウトによる中断

コマンドキューは streaming 実行中に stdout/stderr の出力が一定時間発生しない場合、無出力タイムアウトとしてコマンドを中断しなければならない (MUST)。

無出力タイムアウトの動作は以下の通りとする：
- 出力行（stdout/stderr）の受信時刻を記録する
- 設定された無出力タイムアウト秒数を超えた場合、コマンドを終了させる
- 終了時は警告ログを出力し、エラーメッセージに「inactivity timeout」を含める
- 強制終了は猶予時間を設け、猶予内に終了しない場合は強制 kill する

さらに、無出力タイムアウトで中断されたコマンドは、設定 `command_inactivity_timeout_max_retries` の回数まで自動的に再実行しなければならない (MUST)。

- `command_inactivity_timeout_max_retries` が 0 の場合、再実行は行わない
- `command_inactivity_timeout_max_retries` が未設定の場合、デフォルト値は 3 とする

#### Scenario: 無出力が続いた場合はタイムアウトで中断
- **GIVEN** 無出力タイムアウトが 900 秒に設定されている
- **AND** コマンドが stdout/stderr を一切出力しない
- **WHEN** 900 秒以上無出力が継続する
- **THEN** コマンドはタイムアウトとして中断される
- **AND** エラーメッセージに「inactivity timeout」が含まれる

#### Scenario: 出力があればタイムアウトは延長される
- **GIVEN** 無出力タイムアウトが 60 秒に設定されている
- **WHEN** コマンドが 30 秒ごとに stdout を出力する
- **THEN** 無出力タイムアウトは発生しない

#### Scenario: タイムアウト無効化
- **GIVEN** 無出力タイムアウトが 0 に設定されている
- **WHEN** コマンドが長時間無出力で実行される
- **THEN** 無出力タイムアウトは適用されない

#### Scenario: デフォルトで無出力タイムアウト後に最大3回リトライされる
- **GIVEN** `command_inactivity_timeout_max_retries` が未設定である
- **AND** 無出力タイムアウトが 10 秒に設定されている
- **AND** コマンドが stdout/stderr を一切出力しない
- **WHEN** 10 秒以上無出力が継続する
- **THEN** コマンドは無出力タイムアウトとして中断される
- **AND** コマンドは最大 3 回まで再実行される

#### Scenario: retries=0 の場合は無出力タイムアウト後にリトライしない
- **GIVEN** `command_inactivity_timeout_max_retries` が 0 に設定されている
- **AND** 無出力タイムアウトが 10 秒に設定されている
- **AND** コマンドが stdout/stderr を一切出力しない
- **WHEN** 10 秒以上無出力が継続する
- **THEN** コマンドは無出力タイムアウトとして中断される
- **AND** コマンドは再実行されない
