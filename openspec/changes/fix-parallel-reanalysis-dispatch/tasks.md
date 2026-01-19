## 1. Implementation
- [x] 1.1 実行中の apply/archive バッチ待機ループにキュー通知の監視を追加する（`src/parallel/mod.rs` の `execute_apply_and_archive_parallel` で `DynamicQueue` を監視）
- [x] 1.2 空きスロットがある場合にキューから新規 change を取り込んでワークスペース生成とタスク spawn を行う（`execute_apply_and_archive_parallel` のループで検証）
- [x] 1.3 キュー通知で再分析が走ることをログ/イベントで確認できるようにする（`ParallelEvent::Log` で確認）

## 2. Validation
- [x] 2.1 ユニット/統合テストで、実行中にキューへ追加された change が空きスロットで開始されることを確認する（`src/parallel/mod.rs` のテスト追加、または既存テスト拡張）
- [x] 2.2 `cargo test` で関連テストが通ることを確認する
