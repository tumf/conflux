## Implementation Tasks

- [ ] 1. `src/parallel/mod.rs`: `merge_deferred_changes: HashSet<String>` を `resolve_wait_changes: HashSet<String>` と `merge_wait_changes: HashSet<String>` に分離 (verification: `cargo build` が通る)
- [ ] 2. `src/parallel/builder.rs`: 初期化を2つの空 HashSet に更新 (verification: `cargo build`)
- [ ] 3. `src/parallel/merge.rs` `handle_merge_and_cleanup`: MergeDeferred 受信時に `auto_resumable` で `resolve_wait_changes` または `merge_wait_changes` に振り分け (verification: `cargo test` 既存テスト)
- [ ] 4. `src/parallel/queue_state.rs` `retry_deferred_merges`: 対象を `resolve_wait_changes` のみに変更。成功時は `resolve_wait_changes` から除去、MergeWait 降格時は `merge_wait_changes` に移動 (verification: `cargo test`)
- [ ] 5. `src/parallel/queue_state.rs` `has_merge_deferred` を `has_resolve_wait` にリネームし `resolve_wait_changes` を参照 (verification: `cargo build`)
- [ ] 6. `src/parallel/orchestration.rs`: break 条件 (L164, L193) に `self.resolve_wait_changes.is_empty()` と `self.manual_resolve_active() == 0` を追加 (verification: `cargo test`)
- [ ] 7. `src/tui/state.rs` `handle_all_completed`: Resolving チェックによる Running 維持ワークアラウンド (L1656-1668) を除去 (verification: `cargo test`)
- [ ] 8. `src/parallel/tests/executor.rs`: `merge_deferred_changes` 参照を `resolve_wait_changes` / `merge_wait_changes` に更新 (verification: `cargo test`)
- [ ] 9. 新テスト追加: ResolveWait が残っている間に queue 追加 → scheduler が analyze を発火するシナリオ (verification: `cargo test --test e2e_tests` or unit test)
- [ ] 10. 新テスト追加: MergeWait のみ残り → scheduler loop が正常終了するシナリオ (verification: `cargo test`)
- [ ] 11. `cargo fmt --check && cargo clippy -- -D warnings` で lint 確認

## Future Work

- MergeWait → M キー → resolve 開始時に scheduler loop を再起動する仕組み（現状は AllCompleted 後に M キーで resolve は走るが、その後の queue 追加は処理されない。本 change で ResolveWait 中は scheduler が回るため大部分のケースは解消するが、MergeWait のみで終了後の再起動は別 proposal のスコープ）
