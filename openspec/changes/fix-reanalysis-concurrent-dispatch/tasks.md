## 1. 実装
- [ ] 1.1 スケジューラ状態を追加する（JoinSet / Semaphore / in-flight セット）。完了条件: `ParallelExecutor` に in-flight 追跡があり、`available_slots` が in-flight 数を参照することを `src/parallel/mod.rs` で確認する。
- [ ] 1.2 `execute_with_order_based_reanalysis` を `tokio::select!` ベースの非ブロッキング構造にする。完了条件: `queue.notified()` と `join_set.join_next()` の両方を待機し、dispatch を await していないことを `src/parallel/mod.rs` で確認する。
- [ ] 1.3 dispatch を spawn 関数へ分離し、apply/acceptance/archive を JoinSet に登録する。完了条件: dispatch 後に re-analysis ループが継続することを `src/parallel/mod.rs` で確認する。
- [ ] 1.4 dynamic queue の取り込みをスケジューラ内に集約し、analysis → dispatch の順序を保証する。完了条件: apply 側の動的 spawn が存在しないことを `src/parallel/mod.rs` で確認する。
- [ ] 1.5 re-analysis トリガと slots/in-flight 数のログを追加する。完了条件: ログ出力に trigger 種別（queue/timer/completion）と slots が含まれることを確認する。

## 2. 検証
- [ ] 2.1 `tokio::time::pause` を使った単体テストで「apply 実行中の queue 追加 → re-analysis → slots 空きで dispatch」を確認する。完了条件: 新規テストが `cargo test` で成功する。
- [ ] 2.2 `cargo test` を実行し、並列実行関連の既存テストが通過することを確認する。
