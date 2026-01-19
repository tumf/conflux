## 1. Implementation
- [x] 1.1 archive 前タスク検証で archive フォールバック（worktree内 `openspec/changes/archive/<date>-<change_id>/tasks.md`）を参照するように修正する（`src/execution/archive.rs` の `get_task_progress` または呼び出し元で実装し、実行経路で反映されることを確認）
- [x] 1.2 並列 archive 実行時のタスク検証が worktree 内の archive 済み tasks.md を利用できることを確認する（`src/parallel/executor.rs` の archive ループで `get_task_progress` が fallback を通ることを確認）
- [x] 1.3 タスク検証のユニットテストを追加または更新する（`src/execution/archive.rs` の tests で archive 側 tasks.md フォールバックのケースを追加し、`cargo test` で通ることを確認）

## 2. Validation
- [x] 2.1 `cargo test` を実行して全体テストが通ることを確認する
