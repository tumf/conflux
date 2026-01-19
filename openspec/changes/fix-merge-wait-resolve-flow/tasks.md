## 1. 設計・調査
- [x] 1.1 `execute_with_order_based_reanalysis` のarchive完了後フローを整理し、merge/cleanupの差分を整理する（確認: `src/parallel/mod.rs` のTODO周辺を読み、既存group実装との違いを比較する）

## 2. 実装
- [x] 2.1 order-basedループのarchive完了時に`attempt_merge`を呼び、MergeDeferredなら`MergeWait`に遷移する（確認: `ParallelEvent::MergeDeferred` と `WorkspaceStatus::MergeWait` が送られ、worktreeが残ること）
- [x] 2.2 merge成功時はcleanupイベントと`cleanup_workspace`を実行し、worktreeが削除されること（確認: `CleanupStarted/CleanupCompleted` が発火すること）
- [x] 2.3 resolve/mergeイベントの送出が既存フローと一致するように整理する（確認: `ResolveStarted/ResolveCompleted/ResolveFailed` が conflict 解決時に送られること）
- [x] 2.4 order-basedループ終了時に`WorkspaceCleanupGuard`を正しくcommit/preserveし、MergeWaitのworktreeがDropで削除されないこと（確認: MergeWaitのchangeが残る状態でworktreeが維持されること）

## 3. テスト/検証
- [x] 3.1 既存テストに加え、MergeDeferred時のworktree保持を検証するテストを追加または更新する（確認: `cargo test` で該当テストが通ること）
- [x] 3.2 parallel-execution関連の既存テストが通ることを確認する（確認: `cargo test`）
