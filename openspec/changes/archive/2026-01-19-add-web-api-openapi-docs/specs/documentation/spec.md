## ADDED Requirements
### Requirement: OpenAPI YAML manual
ドキュメントは Web 監視 API の OpenAPI 形式のマニュアルを `docs/` 配下に提供しなければならない（SHALL）。

#### Scenario: OpenAPI YAML が参照可能である
- **WHEN** 利用者が Web UI 用 API の仕様を確認する
- **THEN** `docs/` 配下に OpenAPI 3.1 形式の YAML が存在する
- **AND** `GET /api/health`, `GET /api/state`, `GET /api/changes`, `GET /api/changes/{id}` が記載されている
- **AND** 変更の承認 API と WebSocket `/ws` の仕様が明記されている
