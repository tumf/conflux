## ADDED Requirements

### Requirement: AI command invocationは絶対実行時間上限を持つ

common AI command runnerは`command_max_runtime_secs`を、最初のchild spawn成功時から同一logical invocation全体を覆うabsolute deadlineとして適用しなければならない（MUST）。deadlineはtransport retry、inactivity retry、retry delay、child respawnを跨いで単一であり、stdout/stderr activityまたは後続attemptでresetされてはならない（MUST NOT）。defaultは3,600秒、`0`はdeadline無効化とする（MUST）。runtime-limit expiryはinvocationの全retry admissionを閉じ、既存のgraceful-then-forceful cleanup pathでowned process groupを終了し、typed non-retryable runtime-limit outcomeを返さなければならない（MUST）。

#### Scenario: 継続出力はabsolute deadlineを延長しない

- **GIVEN** `command_max_runtime_secs`が有効である
- **AND** owned AI commandが継続的に出力している
- **WHEN** 最初のchild spawnからの経過時間が設定上限に達する
- **THEN** Confluxはinvocationのretry admissionを閉じる
- **AND** owned process groupを終了してquiescenceを証明する
- **AND** 同じrunでcommandを自動retryしない

#### Scenario: retryはabsolute deadlineをresetしない

- **GIVEN** AI command invocationがabsolute deadline内でretry可能なfailureを返す
- **WHEN** command queueがchildをrespawnする
- **THEN** 後続attemptは最初のchild spawnから計測した残り時間だけを使用する
- **AND** retry回数に応じてabsolute runtime budgetが乗算されない

#### Scenario: Zeroはabsolute deadlineを無効化する

- **GIVEN** `command_max_runtime_secs`が`0`である
- **WHEN** owned AI commandが他のlifecycle制約を満たしたままactiveである
- **THEN** Confluxはtotal elapsed runtimeだけを理由にcommandを終了しない
- **AND** inactivity timeoutとexplicit cancellationは独立して適用される

#### Scenario: runtime expiry後はcleanup証明が必要である

- **GIVEN** AI commandがabsolute runtime limitを超える
- **WHEN** bounded process-group cleanupがquiescenceを証明できない
- **THEN** Confluxはactionable cleanup diagnosticsを返す
- **AND** 正常終了をacknowledgeしない
- **AND** 後続retryをadmitしない

## MODIFIED Requirements

### Requirement: 無出力タイムアウトによる中断

コマンドキューは streaming 実行中に stdout/stderr の出力が一定時間発生しない場合、無出力タイムアウトとしてコマンドを中断しなければならない (MUST)。無出力タイムアウトはabsolute runtime limitとは独立して評価され、出力行は無出力期限だけを延長し、absolute runtime deadlineを延長してはならない（MUST NOT）。

無出力タイムアウトの動作は以下の通りとする：
- 出力行（stdout/stderr）の受信時刻を記録する
- 設定された無出力タイムアウト秒数を超えた場合、コマンドを終了させる
- 終了時は警告ログを出力し、エラーメッセージに「inactivity timeout」を含める
- 強制終了は猶予時間を設け、猶予内に終了しない場合は強制 kill する

#### Scenario: 無出力が続いた場合はタイムアウトで中断

- **GIVEN** 無出力タイムアウトが 900 秒に設定されている
- **AND** コマンドが stdout/stderr を一切出力しない
- **WHEN** 900 秒以上無出力が継続する
- **THEN** コマンドはタイムアウトとして中断される
- **AND** エラーメッセージに「inactivity timeout」が含まれる

#### Scenario: 出力があれば無出力タイムアウトだけが延長される

- **GIVEN** 無出力タイムアウトが 60 秒に設定されている
- **AND** absolute runtime limitが有効である
- **WHEN** コマンドが30秒ごとにstdoutを出力する
- **THEN** 無出力タイムアウトは発生しない
- **AND** absolute runtime deadlineは延長されない

#### Scenario: タイムアウト無効化

- **GIVEN** 無出力タイムアウトが0に設定されている
- **WHEN** コマンドが長時間無出力で実行される
- **THEN** 無出力タイムアウトは適用されない
- **AND** 有効なabsolute runtime limitは引き続き独立して適用される
