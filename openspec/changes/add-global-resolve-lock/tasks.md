# Tasks: add-global-resolve-lock

## 実装タスク

- [x] `src/parallel/mod.rs` に `GLOBAL_MERGE_LOCK` とアクセサ関数 `global_merge_lock()` を追加
- [x] `ParallelExecutor` 構造体から `merge_lock` フィールドを削除
- [x] `ParallelExecutor::new()` から `merge_lock` の初期化を削除
- [x] `ParallelExecutor::with_backend()` から `merge_lock` の初期化を削除
- [x] `ParallelExecutor::with_backend_and_queue_state()` から `merge_lock` の初期化を削除
- [x] `attempt_merge()` で `self.merge_lock` を `global_merge_lock()` に置き換え
- [x] テストコード内の `merge_lock` フィールド初期化を削除
- [x] `cargo fmt` でフォーマット
- [x] `cargo clippy` でリント確認
- [x] `cargo test` で既存テストの動作確認
