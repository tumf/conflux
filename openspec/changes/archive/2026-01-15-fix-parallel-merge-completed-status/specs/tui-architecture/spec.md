# tui-architecture Specification Delta

## MODIFIED Requirements

### Requirement: Event-Driven State Updates

TUI は実行イベントを受信して内部状態を更新しなければならない（SHALL）。

**変更内容**: `MergeCompleted` イベントを処理して、並列モードでマージが完了した変更のステータスを `Archived` に更新する。

#### Scenario: Parallel mode での MergeCompleted イベント受信

- **GIVEN** TUIが並列モードで実行中
- **WHEN** `ExecutionEvent::MergeCompleted { change_id, revision }` イベントを受信する
- **THEN** TUIは `change_id` に該当する変更のステータスを `Archived` に設定する
- **AND** 変更の `elapsed_time` を記録する（`started_at` から経過時間を計算）
- **AND** ログに "Merge completed for '{change_id}'" が追加される

#### Scenario: Archived ステータスは completed として表示される

- **GIVEN** 変更のステータスが `Archived` である
- **WHEN** TUIが変更リストをレンダリングする
- **THEN** 変更は緑色の "completed" (または "archived") として表示される
- **AND** ステータスが `UNCOMMITTED` や `NotQueued` として表示されることはない
