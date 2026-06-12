## MODIFIED Requirements

### Requirement: Non-blocking Merge in Scheduler Loop

パラレルスケジューラの `tokio::select!` イベントループは、workspace 完了後の merge + コンフリクト解決処理によってブロックされてはならない（MUST NOT）。merge + resolve 処理はバックグラウンドタスクとして非同期に実行し、スケジューラループは queued change の dispatch を継続しなければならない（SHALL）。

この非ブロッキング要件は post-archive merge に限らず、すべての base-mutating lane 作業に適用されなければならない（MUST）。具体的には、ResolveWait の deferred merge retry（コンフリクト解決エージェント実行を含む）、RejectWait の rejection-review retry、および手動 resolve（TUI `M` キー由来の reducer ResolveWait promotion）の実行を、スケジューラループタスク内で直接 await してはならない（MUST NOT）。スケジューラループが行ってよいのは promotion（reducer の base-mutating lane への昇格）とバックグラウンドタスクの spawn、および結果の受信処理のみである（MUST）。

スケジューラループタスクは global merge lock の取得を待ってブロックしてはならない（MUST NOT）。merge 試行は resolve アクティブ判定をロック取得より前に評価し、ロックが取得できない場合は自動再開可能な Deferred として返却しなければならない（MUST）。Deferred は既存の merge/resolve 完了トリガで自動的に再評価されなければならない（MUST）。

merge/resolve の結果（成功・Deferred・失敗）はスケジューラループに非同期に通知され、適切に処理されなければならない（MUST）。base-mutating lane の単一占有（同時に最大1つの resolve または rejection review）は reducer の lane 占有状態によって維持されなければならない（MUST）。spawn された retry の実行中は、スケジューラはドレイン完了・persistent idle・終了判定においてその作業を未完了として扱わなければならない（MUST）。

spawn された base-mutating lane retry の結果が Merged 以外（自動再開可能な Deferred、または失敗）である場合、スケジューラは結果受信処理において reducer の base-mutating lane 占有を解放しなければならない（MUST）。自動再開可能な Deferred で終わった change は、promotion 元の wait 種別（ResolveWait / RejectWait）に復元され、以降の merge/resolve 完了トリガまたは queue notification で再 promote 可能でなければならない（MUST）。retry の失敗が `ResolveFailed` / `RejectionReviewFailed` などの失敗イベントを伴わずに終了した場合（例: workspace 喪失）も、lane 占有を解放し、運用者可視のイベントを発行しなければならない（MUST）。lane 占有の解放漏れにより promotion が恒久的に不能となる状態（生存するタスクを伴わない Resolving / Rejecting の残留）を生じさせてはならない（MUST NOT）。retry の失敗は運用者に対して 1 回だけ報告されなければならず（MUST）、retry 本体が発行した失敗イベントに加えて汎用エラーを重複報告してはならない（MUST NOT）。

spawn された retry が実マージを行わずに retry 意図を放棄して終了する場合（give-up: workspace 喪失、stale workspace path、base への既マージ検出による stale intent cleanup を含む）、retry 本体は intent 解除と同時に reducer の lane 占有を同期的に解放しなければならない（MUST）。give-up による解放では、対象 change を ResolveWait / RejectWait のいずれの wait queue にも再登録してはならない（MUST NOT）。give-up の結果が Merged 相当のトリガとしてスケジューラに届いた後、後続の ResolveWait / RejectWait waiter の promotion が可能でなければならない（MUST）。give-up 解放は terminal 遷移済みエントリおよび lane 非占有エントリに対しては no-op でなければならない（MUST）。

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

#### Scenario: Auto-resumable deferred retry releases the base-mutating lane

- **GIVEN** Change B が ResolveWait から promote され、spawn された retry の merge 試行が global merge lock 競合により自動再開可能な Deferred（"Merge lane busy"）で終了する
- **WHEN** スケジューラが retry の Deferred 結果を受信処理する
- **THEN** reducer の base-mutating lane 占有が解放される（Change B の activity が Resolving のまま残留しない）
- **AND** Change B は ResolveWait に復元され、resolve wait queue に重複なく再登録される
- **AND** 後続の merge/resolve 完了トリガまたは queue notification で Change B が再 promote される

#### Scenario: Deferred retry converges after the merge lock is released

- **GIVEN** Change B の retry が "Merge lane busy" の自動再開可能 Deferred で終了し、ResolveWait に復元されている
- **AND** global merge lock を保持していたタスクが完了して Merged 結果がスケジューラに届く
- **WHEN** スケジューラが Merged 結果の受信処理で次の waiter を dispatch する
- **THEN** Change B が promote され retry が再実行される
- **AND** ユーザー操作なしで Change B の merge が完了に到達する

#### Scenario: Retry failure without a failure event still releases the lane

- **GIVEN** Change B が ResolveWait から promote され、spawn された retry が `ResolveFailed` 等の失敗イベントを発行せずに失敗する（例: workspace が見つからない）
- **WHEN** スケジューラが retry の失敗結果を受信処理する
- **THEN** reducer の base-mutating lane 占有が解放される
- **AND** 運用者可視のイベントが 1 回発行される
- **AND** 後続の ResolveWait / RejectWait waiter の promotion が引き続き可能である

#### Scenario: Retry give-up without a merge releases the lane without re-enqueueing

- **GIVEN** Change B が ResolveWait または RejectWait から promote され、spawn された retry が workspace 喪失・stale workspace path・base への既マージ検出のいずれかにより実マージを行わず retry 意図を放棄して Merged 相当の結果を返す
- **WHEN** retry 本体が intent を解除して give-up を確定する
- **THEN** reducer の base-mutating lane 占有が同期的に解放される（Change B の activity が Resolving / Rejecting のまま残留しない）
- **AND** Change B は resolve wait queue / reject wait queue のいずれにも再登録されない
- **AND** give-up 結果の受信処理を契機として、後続の ResolveWait / RejectWait waiter が promote 可能である

#### Scenario: Give-up by the lane occupant unblocks the next waiter

- **GIVEN** Change B と Change C がともに ResolveWait に存在し、Change B が promote されている
- **AND** Change B の workspace が失われており、spawn された retry が give-up する
- **WHEN** give-up の Merged 相当結果がスケジューラの結果受信処理に届く
- **THEN** Change C が promote され、その retry がバックグラウンドタスクとして spawn される
- **AND** Change B は wait queue に存在せず、再 promote されない
