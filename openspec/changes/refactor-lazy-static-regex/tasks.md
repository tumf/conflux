## Implementation Tasks

- [x] 1. 特性化テスト: `cargo test -- --nocapture` を実行し回帰がないことを記録する（verification: 実行ログで全体テスト成功、`src/spec_test_annotations.rs` の `#[cfg(test)]` モジュールは `src/main.rs` の `#![cfg(not(test))]` 制約で実行対象外であることを確認）
- [x] 2. `src/spec_test_annotations.rs` 内の固定パターン `Regex::new().unwrap()` を `std::sync::LazyLock<Regex>` 静的変数に置き換える（verification: `cargo build` 成功、`cargo test -- --nocapture` 成功）
- [x] 3. `src/analyzer.rs` 内に同様の固定パターン `Regex::new().unwrap()` があれば同様に置き換える（verification: `cargo test analyzer -- --nocapture` で analyzer テスト 18 件成功）
- [x] 4. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` をすべて実行して受け入れ条件を検証する

## Future Work

- プロダクションコード全体の `unwrap()` / `expect()` 監査は別 proposal で扱う

## Acceptance #1 Failure Follow-up

- [x] `cargo test spec_test_annotations -- --nocapture` の代わりに、実際に `src/spec_test_annotations.rs` のテスト実行可否を確認し、`src/main.rs` の `#![cfg(not(test))]` により当該テストモジュールが現構成では実行対象外であることを記録した
- [x] `cargo test analyzer -- --nocapture` を実行し、analyzer 関連テスト（18件）が実行・成功することを再記録した
- [x] 実際に実行した検証コマンドと一致するように Implementation Tasks の verification 記述を更新した
