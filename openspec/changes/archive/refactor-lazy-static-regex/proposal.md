---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/specs/code-maintenance/spec.md
  - openspec/specs/testing/spec.md
  - src/spec_test_annotations.rs
---

# Change: spec_test_annotations.rs 内の Regex を遅延初期化に置き換える

**Change Type**: implementation

## Problem / Context

`src/spec_test_annotations.rs` 内の `parse_spec_file` 関数や `find_test_annotations` 関数で、呼び出しのたびに `Regex::new(...).unwrap()` を実行している（4 箇所以上）。

これは以下の問題を含む:
1. **パフォーマンス**: 正規表現のコンパイルは比較的高コストだが、パターンは固定で毎回同じ結果になる
2. **unwrap() の多用**: Rust の慣例として、固定パターンの正規表現は `std::sync::LazyLock` (Rust 1.80+) でコンパイル時に 1 回だけ初期化し、`unwrap()` を関数本体から除去するのが推奨される
3. **同様パターンの散在**: `src/analyzer.rs:191` 等にも同様の `Regex::new().unwrap()` がある

## Proposed Solution

- `Regex::new(...).unwrap()` を `std::sync::LazyLock<Regex>` で静的初期化に置き換える
- 対象: `src/spec_test_annotations.rs` 内の全固定パターン正規表現（4 箇所）
- 同一パターンで `src/analyzer.rs` にも該当箇所があれば同様に適用する

## Acceptance Criteria

- `cargo fmt --check && cargo clippy -- -D warnings && cargo test` がすべて成功する
- 関数本体から `Regex::new().unwrap()` が除去され、モジュールスコープの `LazyLock<Regex>` に置き換わっている
- 既存テストの動作に変更がない

## Out of Scope

- テストコード内の `unwrap()` 除去（テストは panic 許容）
- 正規表現パターン自体の変更
- 他モジュール (remote/, server/ 等) の unwrap 除去
