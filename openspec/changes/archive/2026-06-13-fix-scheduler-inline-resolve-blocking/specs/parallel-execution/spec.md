## MODIFIED Requirements

### Requirement: Non-blocking Merge in Scheduler Loop

パラレルスケジューラの `tokio::select!` イベントループは、workspace 完了後の merge + コンフリクト解決処理によってブロックされてはならない（MUST NOT）。merge + resolve 処理はバックグラウンドタスクとして非同期に実行し、スケジューラループは queued change の dispatch を継続しなければならない（SHALL）。

この非ブロッキング要件は post-archive merge に限らず、すべての base-mutating lane 作業に適用されなければならない（MUST）。具体的には、ResolveWait の deferred merge retry（コンフリクト解決エージェント実行を含む）、RejectWait の rejection-review retry、および手動 resolve（TUI `M` キー由来の reducer ResolveWait promotion）の実行を、スケジューラループタスク内で直接 await してはならない（MUST NOT）。スケジューラループが行ってよいのは promotion（reducer の base-mutating lane への昇格）とバックグラウンドタスクの spawn、および結果の受信処理のみである（MUST）。

スケジューラループタスクは global merge lock の取得を待ってブロックしてはならない（MUST NOT）。merge 試行は resolve アクティブ判定をロック取得より前に評価し、ロックが取得できない場合は自動再開可能な Deferred として返却しなければならない（MUST）。Deferred は既存の merge/resolve 完了トリガで自動的に再評価されなければならない（MUST）。

merge/resolve の結果（成功・Deferred・失敗）はスケジューラループに非同期に通知され、適切に処理されなければならない（MUST）。base-mutating lane の単一占有（同時に最大1つの resolve または rejection review）は reducer の lane 占有状態によって維持されなければならない（MUST）。spawn された retry の実行中は、スケジューラはドレイン完了・persistent idle・終了判定においてその作業を未完了として扱わなければならない（MUST）。

#### Scenario: Queued change dispatched during resolve

- **GIVEN** Change A のコンフリクト解決（resolve）が進行中で、queued に Change B が存在し、利用可能スロットが 1 以上ある
- **WHEN** スケジューラループの次の iteration が実行される
- **THEN** Change B の re-analysis と dispatch が実行される
- **AND** Change A の resolve は並行して継続する

#### Scenario: Merge result delivered after background completion

- **GIVEN** Change A の merge がバックグラウンドタスクで実行中
- **WHEN** merge が成功する
- **THEN** merge 結果がスケジューラループに通知される
- **AND** `retry_deferred_merges` が呼び出され、ResolveWait の change がリトライされる

#### Scenario: Merge deferred delivered after background attempt

- **GIVEN** Change A の merge がバックグラウンドで試行される
- **WHEN** merge が Deferred（resolve 進行中 or base dirty）となる
- **THEN** Deferred イベントがスケジューラループに通知される
- **AND** Change A は resolve_wait_changes または merge_wait_changes に追加される

#### Scenario: Deferred merge retry resolve runs off the scheduler loop

- **GIVEN** Change A が ResolveWait であり、その deferred merge retry がコンフリクト解決エージェントの実行を必要とする
- **WHEN** スケジューラが ResolveWait retry を dispatch する
- **THEN** retry の merge + resolve 実行はバックグラウンドタスクとして spawn される
- **AND** スケジューラループは次の iteration に進み、dynamic queue 取り込み・queue reconciliation・re-analysis を継続する
- **AND** resolve エージェントの実行完了をスケジューラループタスク内で直接 await しない

#### Scenario: Change queued via dynamic queue during active resolve is analyzed promptly

- **GIVEN** Change A の resolve（手動 resolve または deferred merge retry の resolve）が進行中である
- **AND** ユーザーが TUI の `x` キーで Change B を queue に追加する
- **WHEN** スケジューラループが次の iteration を実行する
- **THEN** Change B は Change A の resolve 完了を待たずに scheduler queue へ取り込まれる
- **AND** 通常の debounce 範囲内で Change B の dependency analysis が開始される
- **AND** 再計算した利用可能スロットが 1 以上であれば Change B の apply dispatch が開始される

#### Scenario: Scheduler loop does not park on global merge lock

- **GIVEN** spawn された merge/resolve タスクが global merge lock を保持して resolve エージェントを実行中である
- **AND** ResolveWait または RejectWait の change が存在する
- **WHEN** queue notification により ResolveWait retry dispatch が評価される
- **THEN** スケジューラループタスクは global merge lock の解放を待ってブロックしない
- **AND** merge 試行はロック競合時に自動再開可能な Deferred を返す
- **AND** スケジューラループは re-analysis と diagnostics を継続できる

#### Scenario: Consecutive resolve waiters do not starve analysis

- **GIVEN** ResolveWait の change が複数存在し、それぞれの retry がコンフリクト解決を必要とする
- **AND** queued に ordinary dispatchable な Change C が存在する
- **WHEN** 先行する retry が完了して次の waiter が promote される
- **THEN** 次の retry もバックグラウンドタスクとして実行される
- **AND** Change C の re-analysis は retry の合間または実行中に行われ、retry 連鎖によって無期限に遅延しない

#### Scenario: Scheduler does not exit while spawned retry is in flight

- **GIVEN** spawn された base-mutating lane retry が実行中である
- **AND** queued と in-flight がともに空である
- **WHEN** スケジューラがドレイン完了・終了判定を評価する
- **THEN** スケジューラは終了せず retry の結果通知を待つ
- **AND** 結果受信後に ResolveWait 解消・次 waiter promotion・re-analysis が行われる
