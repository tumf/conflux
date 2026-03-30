## Implementation Tasks

- [x] `src/tui/state.rs` の `resolve_merge()` 即時開始パスで `self.is_resolving = true` を `TuiCommand::ResolveMerge` 返却前に設定する (verification: `cargo test test_resolve_merge` で既存テスト通過)
- [x] 2連続 M 押しのユニットテストを追加: 1回目の `resolve_merge()` 後に `is_resolving == true` を確認し、2回目の呼び出しがキュー追加（ResolveWait）になることを検証 (verification: `cargo test test_resolve_merge_consecutive`)
- [x] `cargo clippy -- -D warnings` と `cargo fmt --check` の通過を確認 (verification: CI lint pass)

## Future Work

- Web UI 側の状態管理における同等の競合条件の調査
