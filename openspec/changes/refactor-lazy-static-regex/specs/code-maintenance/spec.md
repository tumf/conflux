## MODIFIED Requirements

### Requirement: リファクタリング安全性の担保

オーケストレーターはリファクタリング後も既存仕様の挙動を保ち、検証手順で後退がないことを示すために SHALL 検証を通過しなければならない。

#### Scenario: 既存の検証が通過する

- **WHEN** `cargo fmt` / `cargo clippy -- -D warnings` / `cargo test` を実行する
- **THEN** すべて成功する

#### Scenario: 固定パターン正規表現は静的初期化される

- **GIVEN** `src/spec_test_annotations.rs` 内に固定パターンの正規表現がある
- **WHEN** 当該モジュールがロードされる
- **THEN** 正規表現は `std::sync::LazyLock<Regex>` でモジュールスコープに一度だけ初期化される
- **AND** 関数本体に `Regex::new().unwrap()` が存在しない
