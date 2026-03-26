# Change: 設定モジュールを責務単位に分割する

## Why
`src/config/mod.rs` は約2900行規模で、型定義・デフォルト値・読み込み優先順位・検証ロジックが同居しています。設定追加時の影響範囲が広く、レビューやテスト設計の難易度が高い状態です。

## What Changes
- 設定関連コードを責務別に分割し、`mod.rs` をファサード化する
- 既存の設定読み込み優先順位（project/global/default）とデシリアライズ挙動を保持する
- 分割前の挙動を固定するキャラクタリゼーションテストを先に拡充する

## Impact
- Affected specs: `configuration`
- Affected code: `src/config/mod.rs`, `src/config/*`, 関連テスト
- API/CLI互換性: 変更なし

## Acceptance Criteria
- 設定ファイルの優先順位とマージ結果が現行実装と一致する
- `cargo test` で設定関連テストがすべて成功する
- `cargo clippy -- -D warnings` と `cargo fmt --check` が通過する
