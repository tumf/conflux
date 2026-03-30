## Implementation Tasks

- [ ] 1. `src/parallel/merge.rs`: `is_dirty_reason_auto_resumable()` を削除する (verification: `rg is_dirty_reason_auto_resumable` が 0 件)
- [ ] 2. `src/parallel/merge.rs` `attempt_merge()`: `MergeAttempt::Deferred` 生成時に `is_dirty_reason_auto_resumable` の呼び出しを削除し、resolve カウンター（L277-288）で弾かれた場合は auto_resumable=true、base_dirty_reason で弾かれた場合は常に auto_resumable=false とする (verification: `cargo test attempt_merge`)
- [ ] 3. `src/parallel/merge.rs` post-merge handler (L206-218): `is_dirty_reason_auto_resumable` 呼び出しを削除し、`MergeAttempt::Deferred` の reason が "Resolve in progress" を含む場合のみ `resolve_wait_changes` に入れ、それ以外は `merge_wait_changes` に入れる (verification: `cargo test`)
- [ ] 4. `src/tui/command_handlers.rs` (L591-610): dirty check 分岐から `is_dirty_reason_auto_resumable` を削除し、dirty なら常に `ResolveFailed` を送出する (verification: `cargo test command_handlers`)
- [ ] 5. `src/tui/runner.rs` (L758-764): `apply_to_reducer` の条件に `ExecutionEvent::MergeDeferred { .. }` を追加する (verification: `cargo test runner`)
- [ ] 6. `src/parallel/merge.rs`: `is_dirty_reason_auto_resumable` 関連のユニットテストを削除し、新しい判定ロジックのテストを追加する (verification: `cargo test --lib merge`)
- [ ] 7. 全体ビルド・テスト・lint 確認 (verification: `cargo build && cargo test && cargo clippy -- -D warnings && cargo fmt --check`)

## Future Work

- MergeWait 状態での TUI ユーザー体験改善（dirty の具体的な reason 表示など）
