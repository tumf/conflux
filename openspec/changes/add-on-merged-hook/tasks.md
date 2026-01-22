## 1. 実装
- [ ] 1.1 HookType に on_merged を追加し、HookRunner の登録とプレースホルダー表を更新する
  確認: `src/hooks.rs` に `OnMerged` と `on_merged` の設定が追加され、プレースホルダー表に on_merged 列がある
- [ ] 1.2 parallel の merge 完了後に on_merged を実行する
  確認: `src/parallel/mod.rs` で `ParallelEvent::MergeCompleted` 後に `run_hook(HookType::OnMerged, ...)` が呼ばれる
- [ ] 1.3 TUI Worktree の手動マージ完了後に on_merged を実行する
  確認: `src/tui/runner.rs` の `BranchMergeCompleted` 処理に hook 呼び出しがある
- [ ] 1.4 serial(run) モードで merge 完了相当のタイミングに on_merged を実行する
  確認: `src/serial_run_service.rs` の archive 成功後に `OnMerged` 呼び出しが追加される
- [ ] 1.5 テンプレートとヘルプ文を更新する
  確認: `src/templates.rs` に `on_merged` の例が追加されている

## 2. 検証
- [ ] 2.1 `cargo test` を実行し、既存テストが通ることを確認する
  確認: `cargo test` が成功
