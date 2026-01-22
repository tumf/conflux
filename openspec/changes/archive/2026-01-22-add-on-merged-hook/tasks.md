## 1. 実装
- [x] 1.1 HookType に on_merged を追加し、HookRunner の登録とプレースホルダー表を更新する
  確認: `src/hooks.rs` に `OnMerged` と `on_merged` の設定が追加され、プレースホルダー表に on_merged 列がある
- [x] 1.2 parallel の merge 完了後に on_merged を実行する
  確認: `src/parallel/mod.rs` で `ParallelEvent::MergeCompleted` 後に `run_hook(HookType::OnMerged, ...)` が呼ばれる
- [x] 1.3 TUI Worktree の手動マージ完了後に on_merged を実行する
  確認: `src/tui/runner.rs` の `BranchMergeCompleted` 処理に hook 呼び出しがある
- [x] 1.4 serial(run) モードで merge 完了相当のタイミングに on_merged を実行する
  確認: `src/serial_run_service.rs` の archive 成功後に `OnMerged` 呼び出しが追加される
- [x] 1.5 テンプレートとヘルプ文を更新する
  確認: `src/templates.rs` に `on_merged` の例が追加されている

## 2. 検証
- [x] 2.1 `cargo test` を実行し、既存テストが通ることを確認する
  確認: `cargo test` が成功


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  - すべての issue を修正しました：
  - - テンプレートの hooks 例を全フック種別とすべてのプレースホルダーを含むように更新（`src/templates.rs`）
  - - serial モードのフック順序を修正し、`on_merged` が `on_change_end` の後に実行されるように変更（`src/serial_run_service.rs`）
  - - parallel 自動マージ経路で `list_changes_native()` を使用して実際のタスク数を取得するように修正（`src/parallel/mod.rs`: WorkspaceState::Merged と merge_and_resolve_with）
  - - TUI 手動マージ経路で `list_changes_native()` を使用して実際のタスク数を取得するように修正（`src/tui/runner.rs`）
  - 確認: `cargo build` と `cargo test` が成功し、すべてのテスト（858個）が通りました


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  - すべての issue を修正しました：
  - - `src/templates.rs` の全テンプレート（CLAUDE_TEMPLATE、OPENCODE_TEMPLATE、CODEX_TEMPLATE）の hooks 例を、仕様で要求されたすべてのプレースホルダーを含むように更新（`on_start`、`on_change_start`、`on_finish`、`on_error` など）
  - - `src/parallel/mod.rs:3744-3751` の `ParallelExecutor::merge_and_resolve_with` で `list_changes_native()` を使用して実際のタスク数を取得するように修正し、`HookContext::with_change(change_id, completed_tasks, total_tasks)` に変更
  - 確認: `cargo build` と `cargo test` が成功し、すべてのテスト（888個）が通りました
