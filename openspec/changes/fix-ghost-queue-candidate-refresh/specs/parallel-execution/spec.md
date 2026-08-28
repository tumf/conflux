1|## MODIFIED Requirements
2|
3|### Requirement: Parallel Analysis Targeting
4|
5|並列実行のanalysisはqueuedのchangeのみを対象にしなければならない（MUST）。
6|
7|実行中のchangeが存在せず、queuedのchangeも空の場合、システムはオーケストレーションを終了しなければならない（MUST）。
8|
9|analysis対象をqueuedに限定するため、queuedに含まれないchange（例: marked but unsettled、merged済みchange、実行済みchange、削除済みchange）はanalysis対象から除外されなければならない（MUST）。
10|
11|queuedのchangeが空の場合、analysisを実行してはならない（MUST）。ただし reducer-visible queued intent が存在する場合、scheduler は queued が空であると結論する前に reconciliation を試みなければならない（MUST）。
12|
13|re-analysis は完了イベントに依存せず、キュー変化やタイマーなどのトリガで起動可能でなければならない（MUST）。
14|
15|re-analysis はメインの実行ループ進行に依存せず開始できなければならない（MUST）。
16|
17|スロットが空いていない場合でも re-analysis は実行でき、空きができた時点で次のディスパッチが行われなければならない（MUST）。
18|
19|明示的な queue notification により dynamic queue ingestion または reducer reconciliation から新しい loadable queued candidate が scheduler-local queued work に追加された場合、scheduler はその追加を debounce 対象の timer/poll 再確認として扱ってはならない（MUST NOT）。この場合、現在の scheduler iteration が初回でなく、queue debounce timestamp が新しい場合でも、dependency analysis を開始しなければならない（MUST）。
20|
21|Stable mark settlement が explicit queue service へ新しい loadable queued candidate を追加した場合、その DynamicQueue notification は前段落の明示的 queue-addition edge として扱われなければならない（MUST）。scheduler は mark stability の10秒に通常 queue debounce を重ねてはならない（MUST NOT）。
22|
23|ただし、mark snapshot の変更または stability deadline だけでは queued candidate が作成される前に dependency analysis を起動してはならない（MUST NOT）。同一状態で候補追加を伴わない queue wake、timer wake、blocked-only drain、または candidate-unavailable 状態は、既存の debounce / diagnostic dedupe / notification-driven idle policy に従ってよい（MAY）。
24|
25|Scheduler reconciliation は reducer-visible queued work が analysis 対象へ取り込まれない理由を観測可能にしなければならない（SHALL）。ただし、同じ change と同じ理由が scheduler loop ごとに連続する場合、user-visible logs と WARN-level debug log entries への出力は dedupe、rate-limit、または summary 化されなければならない（SHALL）。
26|
27|Reducer-visible queued reconciliation MUST NOT refresh an existing queue debounce timestamp merely because the same reducer-visible queued intent is reconstructed again from repository-visible OpenSpec state. Reconciliation MAY initialize the debounce timestamp when reducer-visible queued work is first reconstructed and no timestamp exists, but repeated rediscovery of the same reducer-owned queued state MUST allow the original debounce window to elapse or must be handled by the existing explicit queue-notification bypass rules.
28|
29|Accepted queue intent and scheduler candidate discovery MUST converge without requiring owner restart when the active OpenSpec catalog changes in the repository after owner startup. An admitted dynamic queue hint that initially cannot load its candidate MUST NOT be permanently consumed while leaving reducer-visible queued intent without scheduler-local work, a retained wake edge, or a typed wait/block state. The scheduler MUST re-evaluate against a fresh repository-visible active-change view and either admit the loadable candidate or explicitly reconcile genuinely unavailable queue intent out of the queued projection. This recovery MUST remain event-driven and MUST NOT add filesystem polling or out-of-worktree durable control state.
30|
31|<!-- Expected canonical result after archive: queued projection cannot remain as ghost work after a transient or genuine candidate catalog miss; the same live owner refreshes repository-visible candidates or explicitly reconciles unavailable intent. -->
32|
33|#### Scenario: missing queued candidate diagnostic is bounded

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

#### Scenario: proposal added after owner startup is admitted without restart
34|
35|- **GIVEN** a live owner started before active proposal `alpha` existed in the base repository
36|- **AND** the proposal is then committed or merged under `openspec/changes/alpha`
37|- **WHEN** an operator marks and starts `alpha` through an API or TUI route
38|- **AND** the first scheduler candidate lookup reports `candidate_not_found`
39|- **THEN** the same owner re-evaluates a fresh repository-visible active-change view
40|- **AND** `alpha` is added to scheduler-local queued work without owner restart
41|- **AND** Apply can advance to an active phase when no independent gate blocks it
42|
43|#### Scenario: transient catalog miss retains an execution edge
44|
45|- **GIVEN** reducer-visible queued intent exists for `alpha`
46|- **AND** an admitted dynamic queue hint for `alpha` has been popped
47|- **WHEN** candidate discovery cannot yet prove whether the active proposal is loadable
48|- **THEN** the scheduler does not discard the only wake edge while retaining a bare queued projection
49|- **AND** a later relevant repository or reducer transition can re-evaluate `alpha`
50|
51|#### Scenario: genuinely absent candidate does not remain a ghost queue
52|
53|- **GIVEN** reducer-visible queued intent exists for `missing`
54|- **AND** a fresh repository-visible active-change view proves `missing` is not loadable
55|- **AND** no archived-dirty repair candidate or typed lane wait applies
56|- **WHEN** scheduler reconciliation classifies the target
57|- **THEN** no Apply process is dispatched
58|- **AND** the reducer-visible state is explicitly reconciled so `missing` does not remain indefinitely as queued pending work
59|- **AND** the diagnostic identifies the unavailable-candidate result without repeating on every loop
60|- **AND** any independent execution mark is preserved unless an existing explicit transition revokes it
61|
