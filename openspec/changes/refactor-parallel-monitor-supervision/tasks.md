## Implementation Tasks

- [ ] 1. `MergeStallMonitor` のインターフェース変更: `CancellationToken` を受け取る代わりに `tokio::sync::mpsc::Sender<MergeStallEvent>` を受け取るように変更する (verification: `spawn_monitor` の引数から `CancellationToken` が消えていること。`src/merge_stall_monitor.rs`)
- [ ] 2. `MergeStallEvent` 型を定義する: `MergeStallDetected { elapsed_minutes, threshold_minutes, base_branch }` を含む enum を `src/merge_stall_monitor.rs` に追加する (verification: 型定義が存在し、monitor から送信できること)
- [ ] 3. `MergeStallMonitor::spawn_monitor` 内で `cancel_token.cancel()` を `event_tx.send(MergeStallDetected {...})` に置き換える (verification: `cancel_token` への直接キャンセルがなくなっていること。`rg 'cancel_token.cancel' src/merge_stall_monitor.rs` が 0 件)
- [ ] 4. `MergeStallMonitor` に自身の停止用 `CancellationToken` を追加: 並列実行終了時に monitor を停止するための token (verification: monitor は外部から停止できるが、その token は並列実行全体の cancel_token とは別物であること)
- [ ] 5. `src/parallel/orchestration.rs` の `execute_with_order_based_reanalysis` を更新: monitor に cancel_token を渡す代わりに event channel を渡す (verification: `orchestration.rs` から `cancel_token.clone()` を monitor に渡す呼び出しが消えていること)
- [ ] 6. `orchestration.rs` のメインループの `tokio::select!` に monitor event channel の受信ブランチを追加: stall イベント受信時は `warn!` ログと `ParallelEvent::Warning` を送信する (verification: stall 検出時に warning ログが出力され、queue 実行は継続すること)
- [ ] 7. 既存テスト `test_monitor_triggers_cancellation_on_stall` を更新: cancel_token が cancel されないことを検証し、代わりに event channel にイベントが送信されることを検証する (verification: `cargo test merge_stall` が pass)
- [ ] 8. 結合テスト追加: stall 検出後も並列実行ループが継続し、change が正常に dispatch されることを検証する (verification: `cargo test` で新規テストが pass)
- [ ] 9. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` が全て pass する (verification: CI 相当のチェックが通ること)

## Future Work

- stall 検出時の policy 設定 (hard-stop / soft-stop / warn-only) を `.cflx.jsonc` で切り替え可能にする
- 将来の monitor (workspace leak, queue stall 等) を同じ event channel パターンで追加する
