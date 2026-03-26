## 1. キャラクタリゼーション
- [ ] 1.1 設定読み込み優先順位（custom/project/global/default）を固定するテストを追加する（確認: `cargo test config::`）
- [ ] 1.2 既存JSONCデシリアライズとデフォルト値適用を固定するテストを追加する（確認: `cargo test config::jsonc`）

## 2. リファクタリング
- [ ] 2.1 `src/config/mod.rs` の責務を型定義・ロード・バリデーション・変換に分割する（確認: ビルド成功）
- [ ] 2.2 公開インターフェースは維持しつつ `mod.rs` を集約エントリに整理する（確認: 既存呼び出し側の変更最小化）

## 3. 回帰確認
- [ ] 3.1 `cargo test` を実行し、設定関連を含むテストが成功する
- [ ] 3.2 `cargo fmt --check` と `cargo clippy -- -D warnings` を実行し、品質ゲートを通過する
