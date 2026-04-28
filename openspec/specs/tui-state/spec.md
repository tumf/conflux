### Requirement: TUI ステータス表示は Reducer から導出される
TUI の Change ステータス表示（文字列・色）は `ChangeRuntimeState::display_status()` および `display_color()` から導出されなければならない（MUST）。

TUI 固有のステータス enum（旧 `QueueStatus`）を保持してはならない（SHALL NOT）。`ChangeState` は表示用の文字列キャッシュ（`display_status_cache`）と色キャッシュ（`display_color_cache`）のみを持ち、これらは Reducer のスナップショットから更新される。

#### Scenario: TUI が Reducer からステータスを読み取る
- **WHEN** TUI が Change のステータスを表示する
- **THEN** `ChangeState.display_status_cache` の文字列が使用される
- **AND** `ChangeState.display_color_cache` の色が使用される
- **AND** `QueueStatus` enum が codebase に存在しない

#### Scenario: イベント受信時のキャッシュ更新
- **WHEN** `OrchestratorEvent::ProcessingStarted` を TUI が受信する
- **THEN** `ChangeState.display_status_cache` が `"applying"` に更新される
- **AND** `ChangeState.display_color_cache` が `Color::Cyan` に更新される

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

A rejected row SHALL NOT participate in execution mark, queue, resume, or new-change affordances. The TUI MUST keep its frontend-visible execution mark cleared (`selected = false`), MUST ignore queue-oriented key operations for that row, and MUST NOT label the row with the `NEW` badge or count it toward `new_change_count`.

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

#### Scenario: Rejected row is not marked as NEW on refresh

- **GIVEN** `fix-auth` is not yet present in the current TUI list
- **AND** `openspec/changes/fix-auth/proposal.md` exists
- **AND** `openspec/changes/fix-auth/REJECTED.md` exists
- **WHEN** the next TUI refresh adds `fix-auth` as a rejected row
- **THEN** `fix-auth` is displayed as `rejected`
- **AND** the row does NOT show the `NEW` badge
- **AND** `new_change_count` does NOT increase because of `fix-auth`

#### Scenario: Marker removal reactivates the change as unselected active row

- **GIVEN** `fix-auth` was previously shown as a `rejected` row
- **AND** the user removes `openspec/changes/fix-auth/REJECTED.md` from the base branch
- **WHEN** the TUI refreshes after `fix-auth` reappears in the active listing
- **THEN** `fix-auth` is shown as `not queued`
- **AND** `fix-auth` remains `selected = false` until explicitly marked again

### Requirement: TUI rejected row is visible but not selectable

When a change directory contains both `openspec/changes/<change_id>/proposal.md` and `openspec/changes/<change_id>/REJECTED.md`, the TUI change list SHALL display that change as a read-only `rejected` row rather than omitting it entirely.

A rejected row SHALL NOT participate in execution mark, queue, resume, or new-change affordances. The TUI MUST keep its frontend-visible execution mark cleared (`selected = false`), MUST ignore queue-oriented key operations for that row, and MUST NOT label the row with the `NEW` badge or count it toward `new_change_count`.

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

#### Scenario: Rejected row is not marked as NEW on refresh

- **GIVEN** `fix-auth` is not yet present in the current TUI list
- **AND** `openspec/changes/fix-auth/proposal.md` exists
- **AND** `openspec/changes/fix-auth/REJECTED.md` exists
- **WHEN** the next TUI refresh adds `fix-auth` as a rejected row
- **THEN** `fix-auth` is displayed as `rejected`
- **AND** the row does NOT show the `NEW` badge
- **AND** `new_change_count` does NOT increase because of `fix-auth`

#### Scenario: Marker removal reactivates the change as unselected active row

- **GIVEN** `fix-auth` was previously shown as a `rejected` row
- **AND** the user removes `openspec/changes/fix-auth/REJECTED.md` from the base branch
- **WHEN** the TUI refreshes after `fix-auth` reappears in the active listing
- **THEN** `fix-auth` is shown as `not queued`
- **AND** `fix-auth` remains `selected = false` until explicitly marked again
