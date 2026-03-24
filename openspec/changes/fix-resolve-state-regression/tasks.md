## Implementation Tasks

- [x] 1. `handle_resolve_failed`にMergedガードを追加: `src/tui/state.rs`の`handle_resolve_failed`関数で、`change.queue_status`が既に`QueueStatus::Merged`の場合はMergeWaitへの退行をスキップし、ログのみ記録する (verification: `cargo test`で既存テストがパスすること、`handle_resolve_failed`にMerged状態のchangeを渡した場合にqueue_statusがMergedのまま維持されるユニットテストを追加)
- [x] 2. `apply_merge_wait_status`にMergedガードを追加: `src/tui/state.rs`の`apply_merge_wait_status`関数のmatchガードに`QueueStatus::Merged`を追加する (verification: Merged状態のchangeが`merge_wait_ids`に含まれていても`MergeWait`に退行しないユニットテストを追加)
- [x] 3. `auto_clear_merge_wait`の防御的ガード確認: `auto_clear_merge_wait`は`QueueStatus::MergeWait`のchangeのみ対象とするため、上記修正により到達しなくなるが、将来の安全性のためにログメッセージを検証する (verification: Merged状態のchangeがauto_clear_merge_wait経由でQueuedに遷移しないことをユニットテストで確認)
- [x] 4. `cargo clippy -- -D warnings`と`cargo fmt --check`がパスすることを確認 (verification: CI lintと同等のチェック)
- [x] 5. `cargo test`で全テストがパスすることを確認 (verification: テスト全件パス)
