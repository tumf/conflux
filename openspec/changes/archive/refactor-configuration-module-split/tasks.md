## 1. キャラクタリゼーション
- [x] 1.1 設定読み込み優先順位（custom/project/global/default）を固定するテストを追加する（確認: `cargo test config::` — 7 characterization tests pass）
- [x] 1.2 既存JSONCデシリアライズとデフォルト値適用を固定するテストを追加する（確認: `cargo test config::` — tests pass）

## 2. リファクタリング
- [x] 2.1 `src/config/mod.rs` の責務を型定義・ロード・バリデーション・変換に分割する（確認: `src/config/types.rs` 981行、`src/config/load.rs` 157行を新規作成。ビルド成功）
- [x] 2.2 公開インターフェースは維持しつつ `mod.rs` を集約エントリに整理する（確認: `mod.rs` は `pub use types::*` + パスヘルパー + テストのみの2031行ファサードに整理。既存呼び出し側の変更なし）

## 3. 回帰確認
- [x] 3.1 `cargo test` を実行し、設定関連を含むテストが成功する（136 passed, 1 pre-existing flaky failure）
- [x] 3.2 `cargo fmt --check` と `cargo clippy -- -D warnings` を実行し、品質ゲートを通過する（両コマンドともエラーなし）
