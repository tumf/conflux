## 1. Implementation
- [ ] 1.1 `ParallelExecutor::execute_apply_and_archive_parallel` の`MergeAttempt::Merged`成功時に`HookRunner::run_hook(HookType::OnMerged, ...)`を追加する（検証: `src/parallel/mod.rs`のマージ成功ブロックにフック呼び出しがあること）。
- [ ] 1.2 `ParallelExecutor::execute_with_order_based_reanalysis`のマージ成功経路で同様の`on_merged`呼び出しを追加する（検証: `src/parallel/mod.rs`の`attempt_merge`成功分岐にフック呼び出しがあること）。
- [ ] 1.3 `HookContext`でchange/task進捗と`workspace_path`を設定し、取得失敗時は0/0を維持する（検証: `HookContext`の`with_change`と`with_parallel_context`が使われていること）。

## 2. Validation
- [ ] 2.1 `hooks.on_merged`を設定した状態でパラレル実行のマージ成功時にフックが走ることを確認する（検証: 手動実行ログまたは追加したテストで`on_merged`実行が確認できること）。
