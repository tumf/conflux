## 1. Implementation
- [ ] 1.1 `src/tui/state.rs`: resolve 実行中に `MergeDeferred` を受信しても、対象 change が現在 `Resolving` の場合は `ResolveWait` に遷移せず待ち行列へ追加しない分岐を追加する。完了条件: `handle_merge_deferred` で現在の resolving change_id を判定し、同一なら `Resolving` を維持する。
- [ ] 1.2 `src/tui/state.rs`: 自己キューイングを防ぐユニットテストを追加する。完了条件: `test_merge_deferred_does_not_queue_current_resolving_change` が追加され、`cargo test test_merge_deferred_does_not_queue_current_resolving_change` が通る。
- [ ] 1.3 `src/tui/state.rs`: resolve 実行中に別 change の `MergeDeferred` がキューされることのテストを追加する。完了条件: `test_merge_deferred_queues_other_change_while_resolving` が追加され、`cargo test test_merge_deferred_queues_other_change_while_resolving` が通る。
- [ ] 1.4 回帰確認: `cargo test test_resolve_queue_auto_start_on_completion` が通ることを確認する。
