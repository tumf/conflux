
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

`openspec/changes/<change-id>/proposal.md` と `openspec/changes/<change-id>/REJECTED.md` が存在する場合、TUI は当該 change を `rejected` の read-only row として表示しなければならない（MUST）。

この row は execution candidate ではなく、queue 操作の対象にしてはならない（MUST NOT）。

#### Scenario: refresh adds rejected row as read-only

- **GIVEN** `fix-auth` が `proposal.md` と `REJECTED.md` を持つ
- **WHEN** TUI の refresh が change 一覧を再構築する
- **THEN** `fix-auth` row は一覧に表示される
- **AND** `display_status_cache` は `rejected` になる
- **AND** `selected` は `false` のまま維持される

#### Scenario: rejected row ignores queue toggles

- **GIVEN** カーソルが `rejected` row にある
- **WHEN** ユーザーが Space または `@` で mark/queue 操作を試みる
- **THEN** row の `selected` は変更されない
- **AND** queue intent を変更するコマンドは発行されない

#### Scenario: rejected row is excluded from F5 start/resume/retry candidate selection

- **GIVEN** `rejected` row が一覧に表示されている
- **WHEN** ユーザーが F5 で start/resume/retry を実行する
- **THEN** `rejected` row は実行対象 ID に含まれない
- **AND** scheduler に投入されない

#### Scenario: marker removal reactivates row as normal change

- **GIVEN** 以前 `rejected` row として表示されていた `fix-auth` から `REJECTED.md` が削除された
- **WHEN** 次回 refresh が active listing を取得する
- **THEN** `fix-auth` は通常 row として再活性化される
- **AND** `display_status_cache` は `not queued` になる
- **AND** `selected` は `false` のままである
