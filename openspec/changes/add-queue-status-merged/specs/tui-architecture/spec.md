# TUI Architecture Spec Delta

## MODIFIED Requirements

### Requirement: Event-Driven State Updates

TUI は実行イベントを受信して内部状態を更新しなければならない（SHALL）。

**変更内容**:
- `MergeCompleted` イベント処理時に、状態を `Archived` ではなく **`Merged`** に設定する
- `QueueStatus` enum に **`Merged`** 状態を追加し、並列モードの最終状態として使用する

#### Scenario: Parallel mode での MergeCompleted イベント受信時に Merged 状態に遷移

- **GIVEN** TUIが並列モードで実行中
- **WHEN** `ExecutionEvent::MergeCompleted { change_id, revision }` イベントを受信する
- **THEN** TUIは `change_id` に該当する変更のステータスを **`Merged`** に設定する
- **AND** 変更の `elapsed_time` を記録する（`started_at` から経過時間を計算）
- **AND** ログに "Merge completed for '{change_id}'" が追加される

#### Scenario: Merged ステータスは terminal state として扱われる

- **GIVEN** 変更のステータスが `Merged` である
- **WHEN** TUIが terminal state をチェックする
- **THEN** `Merged` は `Archived`, `Completed`, `Error` と同様に terminal state として扱われる
- **AND** Progress 更新は実行されない
- **AND** リスト更新時も保持される

#### Scenario: Merged ステータスは明確に表示される

- **GIVEN** 変更のステータスが `Merged` である
- **WHEN** TUIが変更リストをレンダリングする
- **THEN** ステータスが "merged" として表示される
- **AND** 色は `Color::LightBlue` で表示される（青系で "完了かつ統合済み" を表現）
- **AND** チェックボックスは `[x]` でグレーアウト表示される

#### Scenario: Serial モードでは Archived が最終状態として維持される

- **GIVEN** Serial モードで実行中
- **WHEN** 変更がアーカイブされる
- **THEN** 状態は `Archived` となる
- **AND** `Merged` 状態には遷移しない
- **AND** `Archived` が最終状態として扱われる
