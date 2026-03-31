## Implementation Tasks

- [ ] `src/tui/state.rs`: `start_processing()` (L1098-1102) から `is_resolving` ガードチェックを削除 (verification: `cargo test test_start_processing` で resolving 中でも処理開始可能なことを確認)
- [ ] `src/tui/state.rs`: `resume_processing()` (L1176-1180) から `is_resolving` ガードチェックを削除 (verification: `cargo test test_resume_processing` で resolving 中でも再開可能なことを確認)
- [ ] `src/tui/state.rs`: `retry_error_changes()` (L1232-1236) から `is_resolving` ガードチェックを削除 (verification: `cargo test test_retry_error` で resolving 中でもリトライ可能なことを確認)
- [ ] テスト `test_start_processing_blocked_while_resolving` を「resolving 中でも start_processing が成功する」テストに書き換え (verification: `cargo test test_start_processing_blocked_while_resolving`)
- [ ] テスト `test_resume_processing_blocked_while_resolving` を「resolving 中でも resume_processing が成功する」テストに書き換え (verification: `cargo test test_resume_processing_blocked_while_resolving`)
- [ ] テスト `test_retry_error_changes_blocked_while_resolving` を「resolving 中でも retry が成功する」テストに書き換え (verification: `cargo test test_retry_error_changes_blocked_while_resolving`)
- [ ] `cargo clippy -- -D warnings` と `cargo test` で全体の回帰がないことを確認 (verification: CI 相当の全テスト通過)

## Future Work

- `is_resolving` フラグの命名を `is_resolve_serialization_active` 等に変更して役割を明確化する検討
