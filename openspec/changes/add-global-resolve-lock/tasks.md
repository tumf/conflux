# Tasks: add-global-resolve-lock

## 実装タスク

- [ ] `src/parallel/mod.rs` に `GLOBAL_MERGE_LOCK` とアクセサ関数 `global_merge_lock()` を追加
- [ ] `ParallelExecutor` 構造体から `merge_lock` フィールドを削除
- [ ] `ParallelExecutor::new()` から `merge_lock` の初期化を削除
- [ ] `ParallelExecutor::with_backend()` から `merge_lock` の初期化を削除
- [ ] `ParallelExecutor::with_backend_and_queue_state()` から `merge_lock` の初期化を削除
- [ ] `attempt_merge()` で `self.merge_lock` を `global_merge_lock()` に置き換え
- [ ] テストコード内の `merge_lock` フィールド初期化を削除
- [ ] `cargo fmt` でフォーマット
- [ ] `cargo clippy` でリント確認
- [ ] `cargo test` で既存テストの動作確認
