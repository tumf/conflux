## Implementation Tasks

- [ ] 1. 特性化テスト: `cargo test spec_test_annotations` を実行し全テスト通過を記録する（verification: テスト結果ログ）
- [ ] 2. `src/spec_test_annotations.rs` 内の固定パターン `Regex::new().unwrap()` を `std::sync::LazyLock<Regex>` 静的変数に置き換える（verification: `cargo build` 成功、`cargo test spec_test_annotations` 全通過）
- [ ] 3. `src/analyzer.rs` 内に同様の固定パターン `Regex::new().unwrap()` があれば同様に置き換える（verification: `cargo test analyzer` 全通過）
- [ ] 4. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` をすべて実行して受け入れ条件を検証する

## Future Work

- プロダクションコード全体の `unwrap()` / `expect()` 監査は別 proposal で扱う
