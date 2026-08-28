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

Accepted queue intent and scheduler candidate discovery MUST converge without requiring owner restart when the active OpenSpec catalog changes in the repository after owner startup. An admitted dynamic queue hint that initially cannot load its candidate MUST NOT be permanently consumed while leaving reducer-visible queued intent without scheduler-local work, a retained wake edge, or a typed wait/block state. The scheduler MUST re-evaluate against a fresh repository-visible active-change view and either admit the loadable candidate or explicitly reconcile genuinely unavailable queue intent out of the queued projection. This recovery MUST remain event-driven and MUST NOT add filesystem polling or out-of-worktree durable control state.

<!-- Expected canonical result after archive: queued projection cannot remain as ghost work after a transient or genuine candidate catalog miss; the same live owner refreshes repository-visible candidates or explicitly reconciles unavailable intent. -->

#### Scenario: proposal added after owner startup is admitted without restart

- **GIVEN** a live owner started before active proposal `alpha` existed in the base repository
- **AND** the proposal is then committed or merged under `openspec/changes/alpha`
- **WHEN** an operator marks and starts `alpha` through an API or TUI route
- **AND** the first scheduler candidate lookup reports `candidate_not_found`
- **THEN** the same owner re-evaluates a fresh repository-visible active-change view
- **AND** `alpha` is added to scheduler-local queued work without owner restart
- **AND** Apply can advance to an active phase when no independent gate blocks it

#### Scenario: transient catalog miss retains an execution edge

- **GIVEN** reducer-visible queued intent exists for `alpha`
- **AND** an admitted dynamic queue hint for `alpha` has been popped
- **WHEN** candidate discovery cannot yet prove whether the active proposal is loadable
- **THEN** the scheduler does not discard the only wake edge while retaining a bare queued projection
- **AND** a later relevant repository or reducer transition can re-evaluate `alpha`

#### Scenario: genuinely absent candidate does not remain a ghost queue

- **GIVEN** reducer-visible queued intent exists for `missing`
- **AND** a fresh repository-visible active-change view proves `missing` is not loadable
- **AND** no archived-dirty repair candidate or typed lane wait applies
- **WHEN** scheduler reconciliation classifies the target
- **THEN** no Apply process is dispatched
- **AND** the reducer-visible state is explicitly reconciled so `missing` does not remain indefinitely as queued pending work
- **AND** the diagnostic identifies the unavailable-candidate result without repeating on every loop
- **AND** any independent execution mark is preserved unless an existing explicit transition revokes it
