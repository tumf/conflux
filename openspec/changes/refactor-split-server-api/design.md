## Context

`src/server/api.rs` が 8,400 行を超え、10 以上の異なるドメイン責務を含んでいる。並列開発時のマージコンフリクトが頻発し、レビュー・ナビゲーションコストが高い。

## Goals / Non-Goals

- Goals: 責務別ファイル分割による可読性・保守性の向上
- Non-Goals: API の変更、新機能追加、パフォーマンス改善

## Decisions

- **Decision**: 単一ファイルをディレクトリモジュール (`api/mod.rs` + サブファイル) に変換する
  - `build_router` は `mod.rs` に残し、各ハンドラは個別ファイルからインポートする
  - Alternatives: 機能別 crate 分離 → 現時点ではオーバーエンジニアリング

- **Decision**: テストは各サブモジュール内に配置する
  - Alternatives: `tests/server_api/` に外部テスト → テスト対象のプライベート関数にアクセスできなくなる

## Risks / Trade-offs

- 一時的に大量ファイル変更が発生する → 単一 PR で atomic にマージする
- `use` パスが変わる他モジュールとの結合 → `pub use` 再エクスポートで移行コスト最小化

## Open Questions

- なし
