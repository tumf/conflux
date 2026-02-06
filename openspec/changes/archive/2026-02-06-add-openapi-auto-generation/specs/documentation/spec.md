## RENAMED Requirements
- FROM: `### Requirement: OpenAPI YAML manual`
- TO: `### Requirement: OpenAPI YAML generation`

## MODIFIED Requirements
### Requirement: OpenAPI YAML generation
ドキュメントは Web 監視 API の OpenAPI 3.1 形式の YAML をコードから自動生成し、`docs/openapi.yaml` として提供しなければならない（SHALL）。OpenAPI YAML は手動編集してはならない（MUST NOT）。

#### Scenario: 生成コマンドで更新する
- **WHEN** 開発者が `make openapi` を実行する
- **THEN** `docs/openapi.yaml` が最新の仕様で生成される
- **AND** `GET /api/health`, `GET /api/state`, `GET /api/changes`, `GET /api/changes/{id}` の仕様が含まれる
- **AND** 変更の承認 API と WebSocket `/ws` が記載される

#### Scenario: 生成差分を検知する
- **WHEN** API 実装が変更され、生成結果がリポジトリと一致しない
- **THEN** `make check-openapi` は失敗する
- **AND** CI は差分を検知して失敗する
