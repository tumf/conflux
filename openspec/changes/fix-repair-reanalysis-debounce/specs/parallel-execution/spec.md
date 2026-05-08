## MODIFIED Requirements

### Requirement: Queue ingestion and analysis targeting

並列実行の analysis は queued の change のみを対象にしなければならない（MUST）。

キューに追加された change は analysis 実行前に queued 集合へ反映されなければならない（MUST）。

scheduler-local queued 集合は reducer-visible queued intent と reconcile されなければならない（MUST）。reconcile は dynamic queue notification の欠落、dynamic queue pop 後の一時的な candidate load failure、または stale local queue state によって reducer-visible queued change が永続的に analysis 対象外になることを防がなければならない（MUST）。

queued の change が空の場合、analysis を実行してはならない（MUST）。ただし reducer-visible queued intent が存在する場合、scheduler は queued が空であると結論する前に reconcile を試みなければならない（MUST）。

実行中の change が存在せず、queued の change も空の場合、オーケストレーションは完了状態にならなければならない（MUST）。ただし reducer-visible queued intent が存在する場合、その intent が terminal / active / missing などの理由で analysis 対象外であることが確認されるまで完了状態として扱ってはならない（MUST NOT）。

queued に含まれない change（例: merged 済み change、実行済み change、削除済み change）は analysis 対象から除外されなければならない（MUST）。

Archived-dirty repair candidate は workspace-derived repair trigger として扱われなければならない（MUST）。scheduler は同じ unchanged archived-dirty repair candidate の再発見を通常の user/reducer queue addition と同じ debounce 更新として扱ってはならない（MUST NOT）。

#### Scenario: queuedのみがanalysis対象になる

- **GIVEN** queued に change が存在する
- **AND** queued 以外に実行中の change が存在する
- **WHEN** 並列実行が analysis を開始する
- **THEN** analysis 対象は queued の change のみになる

#### Scenario: reducer queued intent が scheduler-local queued に反映される

- **GIVEN** change `beta` has reducer-visible queued intent
- **AND** `beta` is not terminal
- **AND** `beta` is not active or in-flight
- **AND** `beta` can be loaded from active OpenSpec changes
- **AND** scheduler-local queued list does not contain `beta`
- **WHEN** the scheduler reconciles queued candidates before analysis
- **THEN** `beta` is added to scheduler-local queued candidates
- **AND** the next analysis includes `beta`

#### Scenario: dynamic queue notification miss is recoverable

- **GIVEN** change `beta` has reducer-visible queued intent
- **AND** the dynamic queue notification for `beta` was missed or already popped
- **AND** scheduler-local queued list does not contain `beta`
- **WHEN** the scheduler loop next reconciles queued candidates
- **THEN** `beta` is still eligible for analysis through reducer-visible queued intent
- **AND** `beta` does not remain indefinitely queued without analysis solely because the notification was missed

#### Scenario: candidate load failure is observable and retried

- **GIVEN** dynamic queue ingestion sees queued change id `beta`
- **AND** active OpenSpec change loading does not currently return `beta`
- **WHEN** scheduler ingestion skips `beta`
- **THEN** the skip reason is logged or emitted as candidate not found
- **AND** if reducer-visible queued intent for `beta` remains and `beta` later becomes loadable, reconciliation can add `beta` to analysis candidates

#### Scenario: queuedが空ならanalysisを実行しない

- **GIVEN** queued の change が存在しない
- **AND** reducer-visible queued intent も存在しない
- **WHEN** 並列実行が analysis を開始しようとする
- **THEN** analysis を実行しない

#### Scenario: 実行中とqueuedが空なら終了する

- **GIVEN** 実行中の change が存在しない
- **AND** queued の change も空である
- **AND** reducer-visible queued intent も存在しない
- **WHEN** 並列実行ループが次の analysis を開始しようとする
- **THEN** analysis を実行しない
- **AND** オーケストレーションは完了状態になる

#### Scenario: queued外のchangeはanalysis対象から除外される

- **GIVEN** queued に含まれない change が存在する
- **AND** queued には別の change が存在する
- **WHEN** 並列実行が analysis を開始する
- **THEN** queued 外の change は analysis 対象から除外される

#### Scenario: archived dirty repair candidate does not extend debounce indefinitely

- **GIVEN** reducer-visible queued intent is empty
- **AND** an existing worktree for change `alpha` has no active `openspec/changes/alpha` directory
- **AND** the same worktree has an archive entry for `alpha`
- **AND** scheduler reconciliation discovers `alpha` as an archived-dirty repair candidate
- **WHEN** scheduler reconciliation observes the same unchanged repair candidate repeatedly
- **THEN** the scheduler MUST NOT refresh normal queue debounce on every loop for `alpha`
- **AND** repair-driven analysis for `alpha` MUST either bypass debounce or run after one bounded debounce interval
- **AND** analysis MUST NOT be postponed indefinitely by rediscovering `alpha` itself

#### Scenario: repeated unchanged repair reconciliation is bounded

- **GIVEN** scheduler reconciliation observes the same archived-dirty repair candidate set repeatedly
- **AND** no dispatch, completion, merge, archive, resolve, queue addition, or worktree state change occurs
- **WHEN** the scheduler loop evaluates queued candidates multiple times
- **THEN** repeated user-visible repair reconciliation diagnostics MUST be deduped, rate-limited, or summarized
- **AND** unchanged repair rediscovery MUST NOT be treated as new scheduler progress each time
- **AND** the scheduler MUST remain capable of progressing analysis when execution capacity is available
