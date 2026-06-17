## MODIFIED Requirements

### Requirement: Parallel Analysis Targeting

並列実行のanalysisはqueuedのchangeのみを対象にしなければならない（MUST）。

実行中のchangeが存在せず、queuedのchangeも空の場合、システムはオーケストレーションを終了しなければならない（MUST）。

analysis対象をqueuedに限定するため、queuedに含まれないchange（例: merged済みchange、実行済みchange、削除済みchange）はanalysis対象から除外されなければならない（MUST）。

queuedのchangeが空の場合、analysisを実行してはならない（MUST）。ただし reducer-visible queued intent が存在する場合、scheduler は queued が空であると結論する前に reconciliation を試みなければならない（MUST）。

re-analysis は完了イベントに依存せず、キュー変化やタイマーなどのトリガで起動可能でなければならない（MUST）。

re-analysis はメインの実行ループ進行に依存せず開始できなければならない（MUST）。

スロットが空いていない場合でも re-analysis は実行でき、空きができた時点で次のディスパッチが行われなければならない（MUST）。

明示的な queue notification により dynamic queue ingestion または reducer reconciliation から新しい loadable queued candidate が scheduler-local queued work に追加された場合、scheduler はその追加を debounce 対象の timer/poll 再確認として扱ってはならない（MUST NOT）。この場合、現在の scheduler iteration が初回でなく、queue debounce timestamp が新しい場合でも、dependency analysis を開始しなければならない（MUST）。

ただし、同一状態で候補追加を伴わない queue wake、timer wake、blocked-only drain、または candidate-unavailable 状態は、既存の debounce / diagnostic dedupe / notification-driven idle policy に従ってよい（MAY）。

Scheduler reconciliation は reducer-visible queued work が analysis 対象へ取り込まれない理由を観測可能にしなければならない（SHALL）。ただし、同じ change と同じ理由が scheduler loop ごとに連続する場合、user-visible logs と WARN-level debug log entries への出力は dedupe、rate-limit、または summary 化されなければならない（SHALL）。

Reducer-visible queued reconciliation MUST NOT refresh an existing queue debounce timestamp merely because the same reducer-visible queued intent is reconstructed again from repository-visible OpenSpec state. Reconciliation MAY initialize the debounce timestamp when reducer-visible queued work is first reconstructed and no timestamp exists, but repeated rediscovery of the same reducer-owned queued state MUST allow the original debounce window to elapse or must be handled by the existing explicit queue-notification bypass rules.

<!-- Expected canonical result after archive: `Parallel Analysis Targeting` prevents reducer-visible queued reconciliation from starving analysis by repeatedly resetting queue debounce, while preserving explicit queue-addition bypass and blocked-only idle behavior. -->

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
- **WHEN** the operator presses `x` in the TUI Changes view and a `not queued` loadable change is added to scheduler-local queued work through dynamic queue ingestion
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
