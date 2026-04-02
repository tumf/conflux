## Implementation Tasks

- [x] 1. parallel merge 開始前に Git backend の original/base branch を初期化する共通経路を追加する (`src/parallel/merge.rs`, `src/vcs/git/mod.rs`; verification: archived merge path が `original_branch()` 未初期化でも base branch を解決できる)
- [x] 2. archived handoff / deferred merge / dependency resolution の `original_branch()` 依存箇所を見直し、初期化漏れを `MergeWait` や dependency unresolved と誤分類しないよう統一する (`src/parallel/queue_state.rs`, `src/parallel/orchestration.rs`; verification: recover 可能な未初期化は待機状態ではなく merge/dependency 判定継続になる)
- [x] 3. recover 不能な detached HEAD などは user-intervention merge wait ではなく明示的な実行エラーとして報告する (`src/parallel/merge.rs`, `src/vcs/git/mod.rs`; verification: recover 不能ケースのログ/イベントが MergeWait と区別される)
- [x] 4. archived change が base branch 初期化漏れだけで停滞しない回帰テストを追加する (`src/parallel/tests/` または関連 integration tests; verification: archived merge path で `Original branch not initialized` を再現しても self-heal するテストが通る)
- [x] 5. lint / typecheck / test を実行して並列 merge 状態遷移の既存仕様を壊していないことを確認する (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`; verification: 全コマンド成功)

## Future Work

- detached HEAD で parallel 実行を開始した場合の UX 改善（開始前診断強化や案内文改善）
