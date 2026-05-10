### Requirement: TUI ステータス表示は Reducer から導出される

TUI の Change ステータス表示（文字列・色）は `ChangeRuntimeState::display_status()` および `display_color()` から導出されなければならない（MUST）。

TUI 固有のステータス enum（旧 `QueueStatus`）を保持してはならない（SHALL NOT）。`ChangeState` は表示用の文字列キャッシュ（`display_status_cache`）と色キャッシュ（`display_color_cache`）のみを持ち、これらは Reducer のスナップショットまたは同じ workspace/git-derived refresh evidence から更新される。

TUI はログ表示の重複抑制のために transient な観測用状態を保持してよい（MAY）が、その状態を reducer-derived display status、scheduler dispatch、resume routing、acceptance、archive、または next-action decision の入力として使ってはならない（MUST NOT）。

Refresh-time `merge_wait_ids` は、archived-but-not-merged workspace evidence から導出された TUI display synchronization input として扱ってよい（MAY）。TUI はこれを scheduler dispatch、resume routing、acceptance、archive、または next-action decision の入力として使ってはならない（MUST NOT）。

#### Scenario: TUI が Reducer からステータスを読み取る

- **WHEN** TUI が Change のステータスを表示する
- **THEN** `ChangeState.display_status_cache` の文字列が使用される
- **AND** `ChangeState.display_color_cache` の色が使用される
- **AND** `QueueStatus` enum が codebase に存在しない

#### Scenario: イベント受信時のキャッシュ更新

- **WHEN** `OrchestratorEvent::ProcessingStarted` を TUI が受信する
- **THEN** `ChangeState.display_status_cache` が `"applying"` に更新される
- **AND** `ChangeState.display_color_cache` が `Color::Cyan` に更新される

#### Scenario: refresh-derived merge wait corrects stale resolve pending display

- **GIVEN** change `alpha` is displayed as `resolve pending`
- **AND** the refresh loop observes `alpha` as archive-complete but not merged into base
- **WHEN** the TUI handles `OrchestratorEvent::ChangesRefreshed` with `alpha` in `merge_wait_ids`
- **THEN** `alpha` is displayed as `merge wait`
- **AND** the display correction does not enqueue, dispatch, archive, accept, or otherwise route workflow execution

#### Scenario: stale merge wait refresh does not regress terminal display

- **GIVEN** change `alpha` is already displayed as `merged` or `rejected`
- **WHEN** the TUI handles a stale `OrchestratorEvent::ChangesRefreshed` that includes `alpha` in `merge_wait_ids`
- **THEN** `alpha` remains displayed as its terminal state
- **AND** the terminal row is not regressed to `merge wait`

### Requirement: is_resolving scope limitation

`is_resolving` フラグは resolve 操作同士の直列化ガードとしてのみ機能しなければならない（`Resolving` は Change レベルの `ActivityState` であり、Project レベルのロックではない）。同一 Project 内の他の Change に対する apply/accept/archive パイプラインの開始・再開・リトライをブロックしてはならない。

#### Scenario: start_processing succeeds during resolving

- **GIVEN** 同一 Project 内のある Change が `Resolving` 状態である（`is_resolving` が `true`）
- **WHEN** ユーザーが他の Change に対して `start_processing` を実行する
- **THEN** 選択された Change のキュー追加と処理開始が正常に行われる

#### Scenario: resume_processing succeeds during resolving

- **GIVEN** 同一 Project 内のある Change が `Resolving` 状態であり、`AppMode` が `Stopped` である
- **WHEN** ユーザーが `resume_processing` を実行する
- **THEN** マークされた Change が Queued に遷移し処理が再開される

#### Scenario: retry_error_changes succeeds during resolving

- **GIVEN** 同一 Project 内のある Change が `Resolving` 状態であり、`AppMode` が `Error` である
- **WHEN** ユーザーが `retry_error_changes` を実行する
- **THEN** エラー状態の Change が Queued にリセットされリトライが開始される

#### Scenario: request_merge still serialized during resolving

- **GIVEN** 同一 Project 内のある Change が `Resolving` 状態である（`is_resolving` が `true`）
- **WHEN** ユーザーが MergeWait の別の Change に対して M キーを押す
- **THEN** その Change は `resolve_queue` に追加され即時開始はされない（resolve 直列化は維持）

### Requirement: TUI rejected row is visible but not selectable

When a change directory contains both `openspec/changes/<change_id>/proposal.md` and `openspec/changes/<change_id>/REJECTED.md`, the TUI change list SHALL display that change as a read-only `rejected` row rather than omitting it entirely.

A rejected row SHALL NOT participate in execution mark, queue, or resume controls. The TUI MUST keep its frontend-visible execution mark cleared (`selected = false`), MUST ignore queue-oriented key operations for that row, MUST NOT label the row with the `NEW` badge, and MUST visibly present the row's terminal status as `rejected` in both Select and Running mode.

#### Scenario: Rejected change is shown in TUI list

- **GIVEN** `openspec/changes/fix-auth/proposal.md` exists
- **AND** `openspec/changes/fix-auth/REJECTED.md` exists
- **WHEN** the TUI refreshes its change list
- **THEN** `fix-auth` is displayed in the list
- **AND** its display status is `rejected`

#### Scenario: Rejected row cannot gain an execution mark

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI
- **WHEN** the user presses Space on that row
- **THEN** the row remains `selected = false`
- **AND** no x mark is shown for `fix-auth`
- **AND** the display status remains `rejected`

#### Scenario: Rejected row is ignored by queue-oriented actions

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI
- **WHEN** the user invokes queue or resume-oriented actions such as `@` or `F5`
- **THEN** `fix-auth` is not added to the execution queue
- **AND** no execution start is requested for `fix-auth`

#### Scenario: Select mode shows rejected status label

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI Select mode
- **WHEN** the change list row is rendered
- **THEN** the row visibly includes the label `[rejected]`
- **AND** the row does NOT show the `NEW` badge

#### Scenario: Running mode keeps rejected status label

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI Running mode
- **WHEN** the change list row is rendered
- **THEN** the row visibly includes the label `[rejected]`

#### Scenario: Marker removal reactivates the change as unselected active row

- **GIVEN** `fix-auth` was previously shown as a `rejected` row
- **AND** the user removes `openspec/changes/fix-auth/REJECTED.md` from the base branch
- **WHEN** the TUI refreshes after `fix-auth` reappears in the active listing
- **THEN** `fix-auth` is shown as `not queued`
- **AND** `fix-auth` remains `selected = false` until explicitly marked again

### Requirement: TUI rejected row is visible but not selectable

When a change directory contains both `openspec/changes/<change_id>/proposal.md` and `openspec/changes/<change_id>/REJECTED.md`, the TUI change list SHALL display that change as a read-only `rejected` row rather than omitting it entirely.

A rejected row SHALL NOT participate in execution mark, queue, or resume controls. The TUI MUST keep its frontend-visible execution mark cleared (`selected = false`), MUST ignore queue-oriented key operations for that row, MUST NOT label the row with the `NEW` badge, and MUST visibly present the row's terminal status as `rejected` in both Select and Running mode.

#### Scenario: Rejected change is shown in TUI list

- **GIVEN** `openspec/changes/fix-auth/proposal.md` exists
- **AND** `openspec/changes/fix-auth/REJECTED.md` exists
- **WHEN** the TUI refreshes its change list
- **THEN** `fix-auth` is displayed in the list
- **AND** its display status is `rejected`

#### Scenario: Rejected row cannot gain an execution mark

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI
- **WHEN** the user presses Space on that row
- **THEN** the row remains `selected = false`
- **AND** no x mark is shown for `fix-auth`
- **AND** the display status remains `rejected`

#### Scenario: Rejected row is ignored by queue-oriented actions

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI
- **WHEN** the user invokes queue or resume-oriented actions such as `@` or `F5`
- **THEN** `fix-auth` is not added to the execution queue
- **AND** no execution start is requested for `fix-auth`

#### Scenario: Select mode shows rejected status label

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI Select mode
- **WHEN** the change list row is rendered
- **THEN** the row visibly includes the label `[rejected]`
- **AND** the row does NOT show the `NEW` badge

#### Scenario: Running mode keeps rejected status label

- **GIVEN** `fix-auth` is displayed as a `rejected` row in the TUI Running mode
- **WHEN** the change list row is rendered
- **THEN** the row visibly includes the label `[rejected]`

#### Scenario: Marker removal reactivates the change as unselected active row

- **GIVEN** `fix-auth` was previously shown as a `rejected` row
- **AND** the user removes `openspec/changes/fix-auth/REJECTED.md` from the base branch
- **WHEN** the TUI refreshes after `fix-auth` reappears in the active listing
- **THEN** `fix-auth` is shown as `not queued`
- **AND** `fix-auth` remains `selected = false` until explicitly marked again

### Requirement: Scheduler dependency diagnostics are state-transition driven

The scheduler MUST emit dependency blocked/resolved diagnostics and events based on dependency blocker state transitions, not merely because a polling loop re-checked the same queued change.

For each blocked queued change, the scheduler MUST compare the current dependency blocker observation to the last emitted blocker observation for that change. The blocker observation MUST distinguish at least the blocked change id, unresolved dependency ids, and dependency target classes. Equivalent blocker observations MUST be treated as no-ops for diagnostic/event emission.

Any remembered blocker observation state MUST be in-memory and non-authoritative. It MUST NOT be persisted under `~/.local/state/cflx/**`, and it MUST NOT be used to decide scheduling eligibility, resume routing, acceptance routing, archive routing, or next-action behavior.

#### Scenario: unchanged dependency blocker emits once

- **GIVEN** change `feature-b` is queued
- **AND** change `feature-b` is blocked by dependency `feature-a`
- **AND** the scheduler has already emitted a `DependencyBlocked` diagnostic/event for the same blocker observation
- **WHEN** the scheduler loop evaluates `feature-b` again and `feature-a` has not changed dependency class or resolution state
- **THEN** no additional `DependencyBlocked` event is emitted for `feature-b`
- **AND** no additional TUI user-visible dependency blocked log is produced for that unchanged blocker observation

#### Scenario: changed dependency blocker emits again

- **GIVEN** change `feature-b` was previously blocked by dependency `feature-a`
- **WHEN** the blocker observation changes, such as the unresolved dependency set changes or `feature-a` changes from queued to rejected
- **THEN** the scheduler emits a new dependency blocked diagnostic/event for `feature-b`
- **AND** the diagnostic identifies the changed blocker state rather than silently suppressing it

#### Scenario: dependency resolution emits once per blocked transition

- **GIVEN** change `feature-b` previously emitted a dependency blocked diagnostic/event
- **WHEN** its dependencies become resolved
- **THEN** the scheduler emits one `DependencyResolved` event for `feature-b`
- **AND** later scheduler loops do not re-emit `DependencyResolved` while `feature-b` remains unblocked
- **AND** if `feature-b` becomes blocked again later, that later blocked transition can emit a new blocked diagnostic/event

#### Scenario: diagnostic suppression does not control scheduling

- **GIVEN** a dependency blocker observation has been remembered for diagnostic suppression
- **WHEN** the scheduler evaluates which changes are executable
- **THEN** executable selection is still derived from analysis, workspace state, git state, and in-flight execution state
- **AND** deleting external log/state directories such as `~/.local/state/cflx/**` does not change the next action chosen for the same workspace contents

### Requirement: TUI dependency transition logs are idempotent

TUI handling of dependency blocked and dependency resolved events MUST be idempotent for user-visible logs. A duplicate event that does not change the displayed row status MUST NOT append another identical user-visible log entry.

#### Scenario: duplicate dependency blocked event is a TUI log no-op

- **GIVEN** change `feature-b` is already displayed as `blocked` in the TUI
- **WHEN** the TUI receives another dependency blocked event for `feature-b` without a display state transition
- **THEN** the TUI keeps `feature-b` displayed as `blocked`
- **AND** the TUI does not append another identical dependency blocked log entry

#### Scenario: dependency resolved logs only on blocked-to-queued transition

- **GIVEN** change `feature-b` is displayed as `blocked` in the TUI
- **WHEN** the TUI receives a dependency resolved event for `feature-b`
- **THEN** the TUI changes the displayed status to `queued`
- **AND** the TUI appends one dependency resolved log entry
- **WHEN** the TUI receives another dependency resolved event for `feature-b` while it is no longer displayed as `blocked`
- **THEN** the TUI does not append another dependency resolved log entry
