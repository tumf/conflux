## MODIFIED Requirements

### Requirement: Queue ingestion and analysis targeting

並列実行の analysis は queued の change のみを対象にしなければならない（MUST）。

キューに追加された change は analysis 実行前に queued 集合へ反映されなければならない（MUST）。

scheduler-local queued 集合は reducer-visible queued intent と reconcile されなければならない（MUST）。reconcile は dynamic queue notification の欠落、dynamic queue pop 後の一時的な candidate load failure、または stale local queue state によって reducer-visible queued change が永続的に analysis 対象外になることを防がなければならない（MUST）。

実行中の change が存在せず、queued の change も空の場合、オーケストレーションは完了状態にならなければならない（MUST）。ただし reducer-visible queued intent が存在する場合、その intent が terminal / active / missing などの理由で analysis 対象外であることが確認されるまで完了状態として扱ってはならない（MUST NOT）。

queued に含まれない change（例: merged 済み change、実行済み change、削除済み change）は analysis 対象から除外されなければならない（MUST）。

Archived-dirty repair candidate は workspace-derived repair trigger として扱われなければならない（MUST）。scheduler は同じ unchanged archived-dirty repair candidate の再発見を通常の user/reducer queue addition と同じ debounce 更新として扱ってはならない（MUST NOT）。

Reducer-terminal final states such as `merged`, `archived`, and `rejected` MUST be dispatch stop gates for ordinary apply/acceptance/archive work. A stale dynamic queue entry, stale scheduler-local candidate, or reducer reconciliation pass MUST NOT add a final terminal change to scheduler-local queued work or analysis candidates.

Recoverable terminal `error` remains distinct: it MUST NOT be dispatched through ordinary apply/acceptance/archive work unless explicit retry intent clears the terminal error according to reducer rules.

<!-- Expected canonical result after archive: `parallel-execution` will require scheduler queue ingestion, reconciliation, and dispatch selection to treat reducer-terminal final states as ordinary dispatch stop gates while preserving explicit retry semantics for terminal errors. -->

#### Scenario: terminal merged dynamic queue entry is ignored

- **GIVEN** change `alpha` is reducer-terminal `merged`
- **AND** a stale dynamic queue entry for `alpha` is popped
- **WHEN** scheduler dynamic queue ingestion evaluates `alpha`
- **THEN** `alpha` is not added to scheduler-local `queued`
- **AND** `alpha` is not included in dependency analysis candidates
- **AND** apply, acceptance, and archive are not started for `alpha`

#### Scenario: terminal merged dispatch preflight stops archive path

- **GIVEN** change `alpha` is reducer-terminal `merged`
- **AND** stale scheduler-local state attempts to dispatch `alpha`
- **WHEN** `dispatch_change_to_workspace` evaluates preflight guards
- **THEN** dispatch is skipped before workspace acquisition or reuse
- **AND** `execute_archive_in_workspace` is not called for `alpha`
- **AND** no `ArchiveStarted` event is emitted for `alpha`

#### Scenario: terminal error remains explicit retry only

- **GIVEN** change `beta` is reducer-terminal `error`
- **WHEN** ordinary scheduler dispatch evaluates `beta` without explicit retry intent
- **THEN** `beta` is skipped as retry-required
- **AND** apply, acceptance, and archive are not started for `beta`
- **WHEN** explicit retry intent clears the terminal error
- **THEN** `beta` can become eligible for ordinary queued dispatch again
