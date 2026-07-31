## MODIFIED Requirements

### Requirement: TUI Stop Processing with Escape Key

TUIはEsc二度押しによる停止時、現在の実行活動を確認しなければならない（SHALL）。現在のエージェントプロセスまたはin-flight実行が存在する場合は、そのプロセスと子プロセスを確実に終了しなければならない（SHALL）。実行活動が存在せずparallel schedulerが待機しているだけの場合は、scheduler/orchestratorを停止しなければならず（SHALL）、プロセスを強制終了したと表示してはならない（MUST NOT）。

#### Scenario: 強制停止で子プロセスが残らない

- **GIVEN** 現在のエージェントプロセスまたはin-flight実行が存在する
- **WHEN** TUIがStoppingモードでユーザーがEscを再度押す
- **THEN** 現在のエージェントプロセスとその子プロセスが終了する
- **AND** 終了待機がタイムアウトした場合でも、追加の終了処理が行われる
- **AND** ログは実際のforce stopを表示する
- **AND** 変更の状態はNotQueuedに戻る
- **AND** 実行マークは保持される

#### Scenario: 実行プロセスがない待機状態を通常停止する

- **GIVEN** parallel orchestratorは動作中である
- **AND** 対象changeは`MergeWait`、`ResolveWait`、deferred merge、またはscheduler idleで待機している
- **AND** 現在のエージェントプロセスおよびin-flight実行は存在しない
- **WHEN** TUIがStoppingモードでユーザーがEscを再度押す
- **THEN** scheduler/orchestratorは停止する
- **AND** ログは`Processing stopped`を一度だけ表示する
- **AND** `Force stopped`またはプロセス終了を主張するログを表示しない
- **AND** 存在しないプロセスへの終了要求を行わない
- **AND** 変更の状態はNotQueuedに戻る
- **AND** 実行マークは保持される
