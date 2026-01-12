# Tasks: 停止中のタスク完了時にQueued状態になる不具合を修正

## Investigation

- [ ] `src/tui/state/events.rs` の `handle_orchestrator_event()` および `update_changes()` でタスク完了検出時の処理を調査
  - タスクの `completed_tasks` と `total_tasks` が更新される箇所を特定
  - 更新時に `queue_status` が変更される条件を確認
- [ ] `src/tui/runner.rs` でタスク更新イベント発行箇所を確認
  - `ProgressUpdated` イベントが発行されるタイミング
  - タスク完了検出ロジックの有無
- [ ] 停止中（`AppMode::Stopped`）で `QueueStatus::Queued` に遷移する全ての箇所をリストアップ
  - `toggle_selection()` (Space キー) : Line 274-298 - 正常動作
  - `handle_orchestrator_event()` での自動遷移 : 要確認
  - その他の自動キュー追加ロジック

## Implementation

- [ ] タスク完了時の自動キュー追加ロジックを特定し、Stopped モードでは実行しないように修正
  - 条件: `app.mode != AppMode::Stopped` を追加
- [ ] コード内のコメントで、Stopped モードでの動作を明確化
  - "In Stopped mode, task completion does not trigger auto-queue"

## Testing

- [ ] 停止中にタスクが100%完了した場合の状態遷移テストを追加
  - 初期状態: `AppMode::Stopped`, タスク 0/5
  - タスク更新: 5/5 に変更
  - 期待結果: `QueueStatus::NotQueued` を維持
- [ ] 停止中にSpaceキーでキュー追加した場合の動作テスト（既存機能の確認）
  - 初期状態: `AppMode::Stopped`, `NotQueued`
  - 操作: Space キー
  - 期待結果: `QueueStatus::Queued` に遷移
- [ ] Running モードからStopped モードへの遷移時の動作テスト
  - 初期状態: `AppMode::Running`, タスク処理中
  - 操作: Esc キー（停止）
  - 期待結果: 処理中のchangeは `Queued` に戻る（既存仕様）

## Validation

- [ ] `cargo test` で全テストが通ることを確認
- [ ] `cargo clippy` でwarningが出ないことを確認
- [ ] TUI 実機テスト: 停止中にタスクを完了しても `[queued]` にならないことを確認
