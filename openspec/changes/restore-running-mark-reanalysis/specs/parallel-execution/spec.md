## MODIFIED Requirements

### Requirement: Parallel Analysis Targeting

並列実行のanalysisはqueuedのchangeのみを対象にしなければならない（MUST）。

実行中のchangeが存在せず、queuedのchangeも空の場合、システムはオーケストレーションを終了しなければならない（MUST）。

analysis対象をqueuedに限定するため、queuedに含まれないchange（例: marked but unsettled、merged済みchange、実行済みchange、削除済みchange）はanalysis対象から除外されなければならない（MUST）。

queuedのchangeが空の場合、analysisを実行してはならない（MUST）。ただし reducer-visible queued intent が存在する場合、scheduler は queued が空であると結論する前に reconciliation を試みなければならない（MUST）。

re-analysis は完了イベントに依存せず、キュー変化やタイマーなどのトリガで起動可能でなければならない（MUST）。

re-analysis はメインの実行ループ進行に依存せず開始できなければならない（MUST）。

スロットが空いていない場合でも re-analysis は実行でき、空きができた時点で次のディスパッチが行われなければならない（MUST）。

明示的な queue notification により dynamic queue ingestion または reducer reconciliation から新しい loadable queued candidate が scheduler-local queued work に追加された場合、scheduler はその追加を debounce 対象の timer/poll 再確認として扱ってはならない（MUST NOT）。この場合、現在の scheduler iteration が初回でなく、queue debounce timestamp が新しい場合でも、dependency analysis を開始しなければならない（MUST）。

Stable mark settlement が explicit queue service へ新しい loadable queued candidate を追加した場合、その DynamicQueue notification は前段落の明示的 queue-addition edge として扱われなければならない（MUST）。scheduler は mark stability の10秒に通常 queue debounce を重ねてはならない（MUST NOT）。

ただし、mark snapshot の変更または stability deadline だけでは queued candidate が作成される前に dependency analysis を起動してはならない（MUST NOT）。同一状態で候補追加を伴わない queue wake、timer wake、blocked-only drain、または candidate-unavailable 状態は、既存の debounce / diagnostic dedupe / notification-driven idle policy に従ってよい（MAY）。

Scheduler reconciliation は reducer-visible queued work が analysis 対象へ取り込まれない理由を観測可能にしなければならない（SHALL）。ただし、同じ change と同じ理由が scheduler loop ごとに連続する場合、user-visible logs と WARN-level debug log entries への出力は dedupe、rate-limit、または summary 化されなければならない（SHALL）。

Reducer-visible queued reconciliation MUST NOT refresh an existing queue debounce timestamp merely because the same reducer-visible queued intent is reconstructed again from repository-visible OpenSpec state. Reconciliation MAY initialize the debounce timestamp when reducer-visible queued work is first reconstructed and no timestamp exists, but repeated rediscovery of the same reducer-owned queued state MUST allow the original debounce window to elapse or must be handled by the existing explicit queue-notification bypass rules.

<!-- Expected canonical result after archive: stable marks specialize the existing explicit queue-addition edge without dropping diagnostic, debounce, or queued-only analysis guarantees. -->

#### Scenario: missing queued candidate diagnostic is bounded

- **GIVEN** reducer-visible queued intent exists for change `alpha`
- **AND** `alpha` is not loadable from active OpenSpec changes
- **WHEN** scheduler reconciliation runs repeatedly without any relevant state change
- **THEN** `alpha` is not added to scheduler-local queued candidates
- **AND** the `candidate_not_found` reason remains observable at least once or through a summary
- **AND** identical `candidate_not_found` user-visible log entries are not emitted on every scheduler loop
- **AND** identical `candidate_not_found` WARN-level debug log entries are not emitted on every scheduler loop

#### Scenario: loadable queued candidate still reconciles after missing-candidate logging

- **GIVEN** reducer-visible queued intent exists for change `beta`
- **AND** `beta` is loadable from active OpenSpec changes
- **WHEN** scheduler reconciliation evaluates queued candidates
- **THEN** `beta` is added to scheduler-local queued candidates when no active, in-flight, terminal, slot, or debounce condition blocks it
- **AND** missing-candidate diagnostic suppression state does not prevent `beta` from being analyzed

#### Scenario: explicit TUI queue addition bypasses queue debounce

- **GIVEN** parallel execution is already running beyond the first scheduler analysis iteration
- **AND** the queue debounce timestamp is fresh enough that timer-driven reanalysis would normally be deferred
- **WHEN** an admitted TUI operation adds a `not queued` loadable change to scheduler-local queued work through dynamic queue ingestion
- **THEN** dependency analysis starts for the queued candidate without waiting for the debounce period to expire
- **AND** the analysis target set includes queued candidates only

#### Scenario: reducer-visible queue reconciliation bypasses debounce when it adds loadable work

- **GIVEN** reducer-visible queued intent exists for change `gamma`
- **AND** `gamma` is loadable from active OpenSpec changes
- **AND** scheduler-local queued work does not yet contain `gamma`
- **AND** the queue debounce timestamp is fresh enough that timer-driven reanalysis would normally be deferred
- **WHEN** scheduler reconciliation adds `gamma` to scheduler-local queued work
- **THEN** dependency analysis starts for `gamma` without waiting for the debounce period to expire

#### Scenario: repeated reducer-visible reconciliation does not starve debounce

- **GIVEN** reducer-visible queued intent exists for change `epsilon`
- **AND** `epsilon` is loadable from active OpenSpec changes
- **AND** queue debounce timestamp is already set from a prior queue addition
- **WHEN** scheduler reconciliation reconstructs `epsilon` again on repeated scheduler ticks without a new explicit queue edit
- **THEN** the existing queue debounce timestamp is not refreshed solely by that repeated reconciliation
- **AND** the original debounce window can elapse so dependency analysis can run normally

#### Scenario: zero capacity still analyzes explicit queue additions without dispatching

- **GIVEN** all ordinary dispatch slots are occupied or held by resolve/manual work
- **AND** a loadable change `delta` is explicitly added to scheduler-local queued work by dynamic queue ingestion or reducer reconciliation
- **WHEN** the scheduler evaluates the queue notification
- **THEN** dependency analysis starts for `delta`
- **AND** ordinary apply dispatch is suppressed until execution capacity becomes available
- **AND** the suppression is observable through a capacity-gated diagnostic or equivalent event

#### Scenario: queue wake without new candidate may remain debounceable

- **GIVEN** a queue notification wakes the scheduler
- **AND** dynamic queue ingestion and reducer reconciliation do not add any new loadable queued candidate
- **WHEN** the scheduler evaluates reanalysis eligibility
- **THEN** the scheduler may defer analysis according to existing debounce, blocked-only, or notification-driven idle policy

#### Scenario: Unsettled mark is not an analysis target

- **GIVEN** a live scheduler exists
- **AND** a loadable ordinary change is marked but its 10-second stability deadline has not elapsed
- **WHEN** scheduler reanalysis eligibility is evaluated
- **THEN** the marked change is not analyzed unless it is independently reducer-visible as queued
- **AND** the scheduler does not poll because of the mark

#### Scenario: Stable mark queue addition bypasses further queue debounce

- **GIVEN** parallel execution is already beyond the first scheduler analysis iteration
- **AND** an operator mark set has remained unchanged for 10 seconds
- **AND** settlement adds a new loadable change through the explicit queue service
- **AND** the queue debounce timestamp is fresh enough that timer-driven reanalysis would normally be deferred
- **WHEN** DynamicQueue notification is ingested
- **THEN** dependency analysis starts without another debounce period
- **AND** the analysis target set includes queued candidates only

#### Scenario: Stable mark addition analyzes at zero capacity

- **GIVEN** all ordinary dispatch slots are occupied or held by resolve/manual work
- **AND** stable mark settlement adds a loadable change to scheduler-local queued work
- **WHEN** the scheduler evaluates the queue notification
- **THEN** dependency analysis starts for the queued change
- **AND** ordinary apply dispatch is suppressed until execution capacity becomes available
- **AND** the suppression is observable through a capacity-gated diagnostic or equivalent event

#### Scenario: No-op settlement does not manufacture an analysis edge

- **GIVEN** a stable mark snapshot reaches its deadline
- **AND** every marked row is already queued, active, waiting, terminal, unavailable, or otherwise ineligible for ordinary admission
- **WHEN** settlement completes without adding a loadable queued candidate
- **THEN** no queue-addition reanalysis edge is created
- **AND** existing debounce, blocked-only, and persistent-idle behavior remains unchanged

### Requirement: Scheduler Loop Termination

The scheduler loop SHALL NOT terminate while any change is in ResolveWait state (auto-resumable merge deferred) or while a manual resolve is actively running.

The scheduler loop SHALL terminate when all of the following conditions are met:
- `queued` changes list is empty
- `in_flight` changes set is empty
- `resolve_wait_changes` set is empty (no auto-resumable deferred merges pending)
- Manual resolve counter is zero (no resolve commands actively executing)
- `join_set` is empty (no spawned tasks running)

Changes in MergeWait state (requiring user intervention) SHALL NOT prevent scheduler loop termination. An unsettled mark snapshot SHALL NOT add a finite-run termination condition. A persistent scheduler that remains live after its typed idle transition MAY retain and settle the process-local deadline independently of scheduler drain detection.

#### Scenario: ResolveWait prevents scheduler exit

**Given**: All apply/archive tasks have completed
**And**: One change is in ResolveWait state (auto_resumable merge deferred)
**And**: The queued list and in_flight set are empty
**When**: The scheduler loop evaluates its break conditions
**Then**: The scheduler loop SHALL continue running
**And**: Dynamic queue notifications SHALL be processed (new changes can be analyzed and dispatched)

#### Scenario: MergeWait does not prevent scheduler exit

**Given**: All apply/archive tasks have completed
**And**: One change is in MergeWait state (requires user intervention)
**And**: No changes are in ResolveWait state
**And**: Manual resolve counter is zero
**When**: The scheduler loop evaluates its break conditions
**Then**: The scheduler loop SHALL terminate and send AllCompleted

#### Scenario: Queue addition during ResolveWait triggers analysis

**Given**: The scheduler loop is running with one change in ResolveWait
**And**: Run slots are available (in_flight + resolve count < max_parallelism)
**When**: A new change is added to the dynamic queue
**Then**: The scheduler SHALL analyze and dispatch the new change

#### Scenario: Pending mark deadline is abandoned when a finite run terminates

- **GIVEN** a finite scheduler satisfies every existing termination condition
- **AND** a process-local mark stability deadline has not expired
- **WHEN** the scheduler terminates
- **THEN** the unsettled snapshot is discarded
- **AND** no queue addition or delayed process-lifetime barrier is created
- **AND** one operator-visible informational outcome identifies that mark settlement was abandoned because the scheduler ended

#### Scenario: Persistent idle retains a live deadline

- **GIVEN** a persistent scheduler has no queued, in-flight, resolve-wait, manual-resolve, or spawned work
- **AND** an operator mark deadline is pending
- **WHEN** the scheduler publishes its typed idle transition and remains live
- **THEN** presentation may move to Select without cancelling the deadline
- **AND** stable settlement may notify and resume the same scheduler

### Requirement: Non-blocking Merge in Scheduler Loop

パラレルスケジューラの `tokio::select!` イベントループは、workspace 完了後の merge + コンフリクト解決処理によってブロックされてはならない（MUST NOT）。merge + resolve 処理はバックグラウンドタスクとして非同期に実行し、スケジューラループは queued change の dispatch を継続しなければならない（SHALL）。

この非ブロッキング要件は post-archive merge に限らず、すべての base-mutating lane 作業に適用されなければならない（MUST）。具体的には、ResolveWait の deferred merge retry（コンフリクト解決エージェント実行を含む）、RejectWait の rejection-review retry、および手動 resolve（TUI `M` キー由来の reducer ResolveWait promotion）の実行を、スケジューラループタスク内で直接 await してはならない（MUST NOT）。スケジューラループが行ってよいのは promotion（reducer の base-mutating lane への昇格）とバックグラウンドタスクの spawn、および結果の受信処理のみである（MUST）。

Mark stability settlement は base-mutating lane または global merge lock の取得を待ってはならない（MUST NOT）。settlement が ordinary queue candidate を追加した場合、scheduler loop は active resolve の完了を待たず queue ingestion と re-analysis を継続しなければならない（MUST）。

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

#### Scenario: Stable mark queued during active resolve is analyzed promptly

- **GIVEN** Change A の resolve（手動 resolve または deferred merge retry の resolve）が進行中である
- **AND** ユーザーが TUI の Space または bulk `x` で Change B を mark する
- **WHEN** mark set が10秒安定し、explicit queue service が Change B を追加する
- **THEN** Change B は Change A の resolve 完了を待たずに scheduler queue へ取り込まれる
- **AND** Change B の dependency analysis が既存 queue-addition edge から開始される
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

#### Scenario: Mark settlement does not wait on active resolve locks

- **GIVEN** a base-mutating task holds the global merge lock
- **AND** a stable mark deadline expires for an eligible ordinary change
- **WHEN** settlement applies its additive queue plan
- **THEN** settlement does not acquire or wait for the global merge lock
- **AND** the scheduler may ingest and analyze the resulting queue addition while resolve continues
