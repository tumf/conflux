## Implementation Tasks

- [x] `src/parallel/merge.rs` の `attempt_merge()` で `base_dirty_reason()` 前に `auto_resolve_count + manual_resolve_count > 0` をチェックし、resolve 進行中なら `MergeAttempt::Deferred("Resolve in progress for another change")` を返す (verification: `cargo test` 既存テスト通過 + 新規テスト)
- [x] `src/parallel/merge.rs` の `is_dirty_reason_auto_resumable()` を更新し、"Resolve in progress" を含む理由も auto_resumable として判定する (verification: 既存 `is_dirty_reason_auto_resumable` テスト更新)
- [x] `src/tui/command_handlers.rs` の `ResolveMerge` ハンドラで `base_dirty_reason` チェック前に `manual_resolve_count > 0` の防御チェックを追加し、resolve 進行中なら `MergeDeferred { auto_resumable: true }` を送信する (verification: 該当コードパスの確認)
- [x] archive完了→resolve進行中→ResolveWait のシナリオを再現するユニットテストを追加 (verification: `cargo test test_attempt_merge_deferred_when_resolve_active`)
- [x] `cargo clippy -- -D warnings` と `cargo fmt --check` の通過を確認 (verification: CI lint pass)

## Future Work

- Web UI 側の状態管理における同等の修正
