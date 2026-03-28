## Implementation Tasks

- [ ] `handle_merge_deferred()` の戻り値を `Option<TuiCommand>` に変更する (src/tui/state.rs) — verification: `cargo build` 成功
- [ ] `auto_resumable=true && !is_resolving` 分岐で `resolve_queue` に追加し `Some(TuiCommand::ResolveMerge(change_id))` を返す (src/tui/state.rs, handle_merge_deferred) — verification: ロジック確認
- [ ] `handle_event()` 内の `handle_merge_deferred()` 呼び出し箇所で返された `Option<TuiCommand>` を処理する (src/tui/state.rs, handle_event) — verification: `cargo build` 成功
- [ ] 既存テスト `test_auto_resumable_merge_deferred_shows_resolve_wait_not_merge_wait` を更新し resolve_queue への追加と TuiCommand 返却を検証する (src/tui/state.rs tests) — verification: `cargo test test_auto_resumable_merge_deferred` pass
- [ ] 新規テスト `test_auto_resumable_merge_deferred_starts_resolve_when_idle` を追加 (src/tui/state.rs tests) — verification: `cargo test test_auto_resumable_merge_deferred_starts_resolve` pass
- [ ] `cargo test` で全テスト pass を確認する — verification: exit code 0
- [ ] `cargo clippy -- -D warnings` が pass することを確認する — verification: exit code 0

## Future Work

- headless モードでの同様のバグ調査と修正（必要に応じて別プロポーザル）
