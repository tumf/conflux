# Change: 停止中のタスク完了時にQueued状態になる不具合を修正

## Why

TUIで処理を停止した後（`AppMode::Stopped`）、タスクを完了マーク（`[ ]` → `[x]`）すると、changeの状態が `[queued]` になってしまう。

**現在の動作:**
- 停止中にタスクを完了すると → changeが `[queued]` 状態になる

**期待される動作（仕様）:**
- `[queued]` は **実行中のみ** の表記であるべき
- `[x]` は「実行予約」であり、queueに入っていない状態
- 停止中にタスクを完了しても、changeは `[queued]` にならない

**仕様上の定義:**
- `[ ]` : 未承認
- `[@]` : 承認済みだがキューに入っていない (approved but not queued)
- `[x]` : キューに入っている、または実行予約 (queued or scheduled)
- `[queued]` : 実行中のキュー内の状態表示

停止中にタスクが100%完了しても、それは単に「実行準備が整った」状態であり、自動的にキューに追加されるべきではない。

## What Changes

停止中（`AppMode::Stopped`）におけるタスク完了検出時の動作を修正：

1. **タスク完了時の自動キュー追加ロジックを確認**
   - `update_changes()` や `handle_orchestrator_event()` でタスク完了検出時の処理を調査
   - 停止中に自動的に `QueueStatus::Queued` に遷移する箇所を特定

2. **Stopped モードでの状態遷移ルールを明確化**
   - タスクが100%完了しても、停止中は `QueueStatus::NotQueued` を維持
   - ユーザーが明示的にSpaceキーで追加した場合のみ `Queued` に遷移

3. **テストケースの追加**
   - 停止中にタスクが完了した場合の状態遷移テスト
   - Stopped → Running遷移時の動作テスト

## Impact

- **Affected specs**: `cli/spec.md` の Queue management in Stopped mode セクション（既存仕様の明確化のみ）
- **Affected code**: 
  - `src/tui/state/events.rs` (タスク更新時の状態遷移ロジック)
  - `src/tui/state/mod.rs` (停止中のqueue状態管理)
- **Breaking changes**: なし（内部動作の修正のみ）
