## MODIFIED Requirements

### Requirement: TUI Stop Processing with Escape Key

TUIはEsc二度押しによる停止時、現在の実行活動を確認しなければならない（SHALL）。現在のエージェントプロセスまたはin-flight実行が存在する場合は、そのプロセスと子プロセスを確実に終了しなければならない（SHALL）。実行活動が存在せずparallel schedulerが待機しているだけの場合は、scheduler/orchestratorを停止しなければならず（SHALL）、プロセスを強制終了したと表示してはならない（MUST NOT）。進行中のbackground mergeまたはbase-lane mutationは安全な停止境界まで完了を待たなければならないが（SHALL）、それ自体をエージェントプロセスのforce stopと表示してはならない（MUST NOT）。

TUIはkeyboard stop、SIGINT、SIGTERMを同じ有界なrun-supervisor shutdown境界へ収束させなければならない（MUST）。agent executionがactiveな場合、shutdownはcommand admissionを閉じ、runをcancelし、owned process groupを終了してquiescenceを証明し、interruption-recovery policyに従ってdirty Apply progressを保存してから終了しなければならない（MUST）。external signalはchild cleanupまたはWIP保存を迂回してはならない（MUST NOT）。cleanupまたは保存を証明できない場合、TUI processはactionable diagnosticsを伴うnon-zeroで終了しなければならない（MUST）。

#### Scenario: 強制停止で子プロセスが残らない

- **GIVEN** 現在のエージェントプロセスまたはin-flight実行が存在する
- **WHEN** TUIがStoppingモードでユーザーがEscを再度押す
- **THEN** command admissionが閉じられる
- **AND** 現在のエージェントプロセスとその子プロセスが終了する
- **AND** 終了待機がタイムアウトした場合でも、追加の終了処理が行われる
- **AND** process-group quiescenceが確認される
- **AND** dirty Apply progressは終了前にWIP snapshotへ保存される
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
- **AND** 遅延した停止イベントが到着しても`Processing stopped`を重複表示しない
- **AND** 変更の状態はNotQueuedに戻る
- **AND** 実行マークは保持される

#### Scenario: 進行中background mergeを安全に停止する

- **GIVEN** parallel orchestratorは進行中のbackground mergeまたはbase-lane mutationを所有している
- **AND** 現在のエージェントプロセスおよびin-flight agent executionは存在しない
- **WHEN** TUIがStoppingモードでユーザーがEscを再度押す
- **THEN** operator cancellationが要求される
- **AND** terminal stopはmergeまたはbase-lane operationが既存の安全な結果境界へ到達するまで待つ
- **AND** ログは`Force stopped`またはエージェントプロセス終了を主張しない
- **AND** cancellation待機が有界期限へ到達してもexecution failureとは分類しない

#### Scenario: SIGTERMはTUI shutdown境界を使用する

- **GIVEN** `cflx tui`がactive Apply commandとdescendant processを所有している
- **WHEN** TUI processがSIGTERMを受信する
- **THEN** signalは即時process exitではなくsupervisor cancellationを要求する
- **AND** retryとspawn admissionが閉じられる
- **AND** 全owned process groupが終了しquiescenceが証明される
- **AND** dirty Apply progressはTUI終了前に保存される

#### Scenario: active runがないsignalは通常停止として扱う

- **GIVEN** `cflx tui`にactive run、owned agent process、in-flight executionが存在しない
- **WHEN** TUI processがSIGINTまたはSIGTERMを受信する
- **THEN** TUIは存在しないprocessへの終了要求を行わず終了する
- **AND** ログはforce stopを主張しない

#### Scenario: 二度目のsignalは保存境界を迂回しない

- **GIVEN** 最初のsignalによるTUI shutdownがowned process groupをcleanup中である
- **WHEN** TUIが二度目のSIGINTまたはSIGTERMを受信する
- **THEN** cleanupはforceful escalationを要求できる
- **AND** process-group quiescence証明を迂回しない
- **AND** dirty Apply progressのWIP保存を迂回しない

#### Scenario: SIGINT cleanup失敗を可視化する

- **GIVEN** `cflx tui`がactive execution中にSIGINTを受信する
- **WHEN** bounded cleanupがowned process groupの空状態を証明できない
- **THEN** TUIはcleanup diagnosticsを伴うnon-zeroで終了する
- **AND** processingが正常停止したと主張しない
- **AND** workspace contentsをrecovery用に保持する
