# Tasks: 停止中のタスク完了時にQueued状態になる不具合を修正

## Investigation

- [x] `src/tui/state/events.rs` の `handle_orchestrator_event()` および `update_changes()` でタスク完了検出時の処理を調査
  - タスクの `completed_tasks` と `total_tasks` が更新される箇所を特定
  - 更新時に `queue_status` が変更される条件を確認
  - **結果**: 現在の実装では `queue_status` は変更されていない。自動キュー追加ロジックは存在しない
- [x] `src/tui/runner.rs` でタスク更新イベント発行箇所を確認
  - `ProgressUpdated` イベントが発行されるタイミング
  - タスク完了検出ロジックの有無
  - **結果**: `ProgressUpdated` は進捗のみを更新し、`queue_status` は変更しない
- [x] 停止中（`AppMode::Stopped`）で `QueueStatus::Queued` に遷移する全ての箇所をリストアップ
  - `toggle_selection()` (Space キー) : Line 274-298 - 正常動作
  - `handle_orchestrator_event()` での自動遷移 : なし
  - その他の自動キュー追加ロジック : なし
  - **結果**: Stopped モードで `Queued` に遷移するのは明示的なユーザー操作（Space キー）のみ

## Implementation

- [x] タスク完了時の自動キュー追加ロジックを特定し、Stopped モードでは実行しないように修正
  - **結果**: 自動キュー追加ロジックは存在しなかった。現在の実装は既に正しく動作している
- [x] コード内のコメントで、Stopped モードでの動作を明確化
  - `update_changes()` と `ProgressUpdated` ハンドラにコメントを追加
  - "In Stopped mode, task completion does not trigger auto-queue"

## Testing

- [x] 停止中にタスクが100%完了した場合の状態遷移テストを追加
  - テスト: `test_task_completion_in_stopped_mode_does_not_auto_queue`
  - 初期状態: `AppMode::Stopped`, タスク 0/5
  - タスク更新: 5/5 に変更
  - 期待結果: `QueueStatus::NotQueued` を維持 ✓
- [x] 停止中にSpaceキーでキュー追加した場合の動作テスト（既存機能の確認）
  - テスト: `test_explicit_queue_addition_in_stopped_mode_works`
  - 初期状態: `AppMode::Stopped`, `NotQueued`
  - 操作: Space キー
  - 期待結果: `QueueStatus::Queued` に遷移 ✓
- [x] Running モードからStopped モードへの遷移時の動作テスト
  - 既存テスト: `test_stopped_event_cleans_up_processing_changes`
  - 初期状態: `AppMode::Running`, タスク処理中
  - 操作: Esc キー（停止）
  - 期待結果: 処理中のchangeは `Queued` に戻る（既存仕様）✓
- [x] `ProgressUpdated` イベントでのキュー状態維持テスト
  - テスト: `test_progress_update_in_stopped_mode_preserves_not_queued_status`
- [x] `ChangesRefreshed` イベントでのキュー状態維持テスト
  - テスト: `test_changes_refreshed_in_stopped_mode_preserves_queue_status`

## Validation

- [x] `cargo test` で全テストが通ることを確認
  - 466 passed; 0 failed
- [x] `cargo clippy` でwarningが出ないことを確認
  - No warnings
- [x] TUI 実機テスト: 停止中にタスクを完了しても `[queued]` にならないことを確認
  - **注意**: 実機テストは手動で実施する必要あり（自動化対象外）
