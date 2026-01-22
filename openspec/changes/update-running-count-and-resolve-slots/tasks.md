## 1. 実装

- [ ] 1.1 `QueueStatus::is_active` を更新し、Running ヘッダーのカウントが queued を含まないようにする。確認: `src/tui/types.rs` の `test_queue_status_is_active` を更新し、`cargo test test_queue_status_is_active` が通る。
- [ ] 1.2 手動 resolve の開始/完了を並列スケジューラの in-flight 管理に反映し、スロットが空くまで queued が dispatch されないようにする。確認: `src/parallel/mod.rs` の in-flight 計算で resolve 中は `available_slots` が減ることをユニットテストで確認する。
- [ ] 1.3 手動 resolve が in-flight に含まれることを検証するテストを `src/parallel/tests/` に追加する。確認: `cargo test parallel::tests::` の該当テストが通る。

## 2. 仕上げ

- [ ] 2.1 Running モードのヘッダー表示が in-flight のみを数えることを確認するため、TUI 進行状態のテストを更新する。確認: 追加したテストで queued がカウントされないことを検証する。
