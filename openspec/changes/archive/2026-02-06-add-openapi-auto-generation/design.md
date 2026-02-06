## Context
- OpenAPI YAML が手動更新のため実装と乖離しやすい
- 仕様は `docs/` 配下を求める一方、現状の配置と参照が不整合

## Goals / Non-Goals
- Goals: コードからの自動生成、単一の配置先、CI での更新必須化
- Non-Goals: API の振る舞い変更、エンドポイント再設計、ドキュメントポータル化

## Decisions
- Rust の OpenAPI 生成クレート（例: utoipa）を利用し、既存の HTTP ハンドラに対応する定義を追加する
- 生成物は `docs/openapi.yaml` を正とし、手動編集は禁止する
- `make openapi` と `make check-openapi` を提供し、CI で差分検知を行う

## Risks / Trade-offs
- スキーマ定義の保守コストが増える
- 生成依存追加によりビルド時間が増える可能性がある

## Migration Plan
- 生成コマンド導入後に `docs/openapi.yaml` を生成し、参照先を統一する
- CI に `make check-openapi` を追加して更新必須化する

## Open Questions
- なし
