## MODIFIED Requirements

### Requirement: 無出力タイムアウトによる中断

コマンドキューは streaming 実行中に stdout/stderr の出力が一定時間発生しない場合、無出力タイムアウトとしてコマンドを中断しなければならない (MUST)。

無出力タイムアウトの動作は以下の通りとする：

- 出力行（stdout/stderr）の受信時刻を記録する
- 設定された無出力タイムアウト秒数を超えた場合、コマンドを終了させる
- 終了時は警告ログを出力し、エラーメッセージに「inactivity timeout」を含める
- 強制終了は猶予時間を設け、猶予内に終了しない場合は強制 kill する

加えて、タイムアウトの原因究明が可能になるよう、無出力タイムアウト発火時および終了処理の各ステップで、少なくとも以下の診断情報をログに含めなければならない (MUST)：

- timeout 秒数（`command_inactivity_timeout_secs`）
- grace 秒数（`command_inactivity_kill_grace_secs`）
- 対象の change id（存在する場合）と operation（apply/acceptance/archive/resolve 等）
- 実行ディレクトリ（cwd）
- 子プロセスの pid と、利用している場合は process group id (pgid)
- 最後に stdout/stderr を受信してからの経過秒数（last activity age）

強制 kill が失敗した場合（例: `EPERM`）、その失敗は警告ログとして記録され、errno とコンテキスト（signal/target pid/pgid）を含めなければならない (MUST)。

#### Scenario: 無出力が続いた場合はタイムアウトで中断

- **GIVEN** 無出力タイムアウトが 900 秒に設定されている
- **AND** コマンドが stdout/stderr を一切出力しない
- **WHEN** 900 秒以上無出力が継続する
- **THEN** コマンドはタイムアウトとして中断される
- **AND** エラーメッセージに「inactivity timeout」が含まれる
- **AND** ログに timeout 秒数、grace 秒数、pid/pgid、last activity age が含まれる

#### Scenario: 出力があればタイムアウトは延長される

- **GIVEN** 無出力タイムアウトが 60 秒に設定されている
- **WHEN** コマンドが 30 秒ごとに stdout を出力する
- **THEN** 無出力タイムアウトは発生しない

#### Scenario: タイムアウト無効化

- **GIVEN** 無出力タイムアウトが 0 に設定されている
- **WHEN** コマンドが長時間無出力で実行される
- **THEN** 無出力タイムアウトは適用されない

#### Scenario: 強制 kill が失敗した場合も診断情報が残る

- **GIVEN** 無出力タイムアウトが発火し、猶予時間が経過した
- **AND** 強制 kill（例: SIGKILL）が `EPERM` で失敗する
- **WHEN** 強制 kill の実行結果が取得される
- **THEN** 警告ログに signal と target pid/pgid が含まれる
- **AND** 警告ログに errno（`EPERM`）が含まれる
