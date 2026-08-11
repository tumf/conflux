## MODIFIED Requirements

### Requirement: Parallel Analysis Targeting

並列実行のanalysisはqueuedのchangeのみを対象にしなければならない（MUST）。

実行中のchangeが存在せず、queuedのchangeも空の場合、システムはオーケストレーションを終了しなければならない（MUST）。persistent lifetime では、未settleのRunning mark snapshotが存在する間はこのsnapshotをprocess-local pending control workとして扱い、stability deadlineまたは新しいmark outcomeを待たなければならない（MUST）。ただし、そのsnapshot自体をqueued analysis targetとして扱ってはならない（MUST NOT）。

analysis対象をqueuedに限定するため、queuedに含まれないchange（例: marked but unsettled、merged済みchange、実行済みchange、削除済みchange）はanalysis対象から除外されなければならない（MUST）。

queuedのchangeが空の場合、analysisを実行してはならない（MUST）。ただし reducer-visible queued intent が存在する場合、scheduler は queued が空であると結論する前に reconciliation を試みなければならない（MUST）。

re-analysis は完了イベントに依存せず、キュー変化やタイマーなどのトリガで起動可能でなければならない（MUST）。

re-analysis はメインの実行ループ進行に依存せず開始できなければならない（MUST）。

スロットが空いていない場合でも re-analysis は実行でき、空きができた時点で次のディスパッチが行われなければならない（MUST）。

Running mark stability reconciliationが10秒settlement後にexplicit queue serviceへ新しいloadable queued candidateを追加した場合、そのDynamicQueue notificationは既存の明示的queue-addition edgeとして扱われなければならない（MUST）。schedulerはmark stabilityの10秒に加えて通常queue debounceを重ねてはならず（MUST NOT）、現在のiterationが初回でなくqueue debounce timestampが新しい場合でもdependency analysisを開始しなければならない（MUST）。

ただし、mark snapshotの変更またはstability deadlineだけでは、queued candidateが作成される前にdependency analysisを起動してはならない（MUST NOT）。同一状態で候補追加を伴わないqueue wake、timer wake、blocked-only drain、またはcandidate-unavailable状態は、既存のdebounce / diagnostic dedupe / notification-driven idle policyに従ってよい（MAY）。

Scheduler reconciliation は reducer-visible queued work が analysis 対象へ取り込まれない理由を観測可能にしなければならない（SHALL）。ただし、同じ change と同じ理由が scheduler loop ごとに連続する場合、user-visible logs と WARN-level debug log entries への出力は dedupe、rate-limit、または summary 化されなければならない（SHALL）。

Reducer-visible queued reconciliation MUST NOT refresh an existing queue debounce timestamp merely because the same reducer-visible queued intent is reconstructed again from repository-visible OpenSpec state. Reconciliation MAY initialize the debounce timestamp when reducer-visible queued work is first reconstructed and no timestamp exists, but repeated rediscovery of the same reducer-owned queued state MUST allow the original debounce window to elapse or must be handled by the existing explicit queue-notification bypass rules.

<!-- Expected canonical result after archive: stable Running marks create explicit queue additions after 10 seconds, then reuse immediate candidate-addition analysis without a second debounce. -->

#### Scenario: Unsettled mark is not an analysis target

- **GIVEN** parallel execution is running
- **AND** a loadable ordinary change is marked but its 10-second mark stability deadline has not elapsed
- **WHEN** scheduler reanalysis eligibility is evaluated
- **THEN** the marked change is not analyzed unless it is independently reducer-visible as queued
- **AND** the scheduler waits for an event or deadline rather than polling because of the mark

#### Scenario: Stable mark queue addition bypasses further queue debounce

- **GIVEN** parallel execution is already beyond the first scheduler analysis iteration
- **AND** a Running mark snapshot has remained unchanged for 10 seconds
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

#### Scenario: Stable mark addition during resolve is analyzed promptly

- **GIVEN** Change A resolve is active
- **AND** Change B is an ordinary loadable `not queued` change
- **WHEN** the operator marks Change B and the final mark set remains unchanged for 10 seconds
- **THEN** Change B enters scheduler-local queued work without waiting for Change A resolve to complete
- **AND** dependency analysis starts from the resulting queue-addition edge
- **AND** Change B is dispatched when recomputed ordinary capacity is available

#### Scenario: No-op settlement does not manufacture an analysis edge

- **GIVEN** a Running mark snapshot reaches its stability deadline
- **AND** every marked row is already queued, active, waiting, terminal, unavailable, or otherwise ineligible for ordinary admission
- **WHEN** settlement completes without adding a loadable queued candidate
- **THEN** no queue-addition reanalysis edge is created
- **AND** existing debounce, blocked-only, and persistent-idle behavior remains unchanged

### Requirement: Non-blocking Merge in Scheduler Loop

パラレルスケジューラの `tokio::select!` イベントループは、workspace 完了後の merge + コンフリクト解決処理によってブロックされてはならない（MUST NOT）。merge + resolve 処理はバックグラウンドタスクとして非同期に実行し、スケジューラループは queued change の dispatch を継続しなければならない（SHALL）。

この非ブロッキング要件は post-archive merge に限らず、すべての base-mutating lane 作業に適用されなければならない（MUST）。具体的には、ResolveWait の deferred merge retry（コンフリクト解決エージェント実行を含む）、RejectWait の rejection-review retry、および手動 resolve（TUI `M` キー由来の reducer ResolveWait promotion）の実行を、スケジューラループタスク内で直接 await してはならない（MUST NOT）。スケジューラループが行ってよいのは promotion（reducer の base-mutating lane への昇格）とバックグラウンドタスクの spawn、および結果の受信処理のみである（MUST）。

Running mark stability deadlineおよびそのsettlementもbase-mutating laneやglobal merge lockを待ってはならない（MUST NOT）。settlementがordinary queue candidateを追加した場合、scheduler loopはactive resolveの完了を待たずにqueue ingestionとre-analysisを継続しなければならない（MUST）。

スケジューラループタスクは global merge lock の取得を待ってブロックしてはならない（MUST NOT）。merge 試行は resolve アクティブ判定をロック取得より前に評価し、ロックが取得できない場合は自動再開可能な Deferred として返却しなければならない（MUST）。Deferred は既存の merge/resolve 完了トリガで自動的に再評価されなければならない（MUST）。

merge/resolve の結果（成功・Deferred・失敗）はスケジューラループに非同期に通知され、適切に処理されなければならない（MUST）。base-mutating lane の単一占有（同時に最大1つの resolve または rejection review）は reducer の lane 占有状態によって維持されなければならない（MUST）。spawn された retry の実行中は、スケジューラはドレイン完了・persistent idle・終了判定においてその作業を未完了として扱わなければならない（MUST）。

<!-- Expected canonical result after archive: active resolve remains non-blocking while stable Running marks can add ordinary queued work and trigger analysis after their settle interval. -->

#### Scenario: Stable marked change is analyzed during active resolve

- **GIVEN** Change A のresolveが進行中である
- **AND** ユーザーがTUIのSpaceまたはbulk `x`でChange Bをmarkする
- **WHEN** final mark setが10秒安定し、Change Bがexplicit queue serviceを通じてqueueへ追加される
- **THEN** Change BはChange Aのresolve完了を待たずscheduler queueへ取り込まれる
- **AND** queue-addition edgeからChange Bのdependency analysisが開始される
- **AND** 再計算した利用可能スロットが1以上であればChange Bのapply dispatchが開始される

#### Scenario: Resolve continues while mark settlement waits

- **GIVEN** Change A のresolveが進行中である
- **AND** Running mark snapshotの10秒stability deadlineがpendingである
- **WHEN** scheduler loopがresolve結果、queue notification、またはmark deadlineを待つ
- **THEN** Change Aのresolveはmark settlementによって停止またはキャンセルされない
- **AND** scheduler loopはglobal merge lockまたはresolve完了を待ってparkしない
