## MODIFIED Requirements

### Requirement: API v1 を提供する
サーバは `api/v1` 名前空間でプロジェクト管理と実行制御の API を提供しなければならない（SHALL）。

#### Scenario: プロジェクト管理 API が応答する
- **WHEN** クライアントが `GET /api/v1/projects` を呼び出す
- **THEN** サーバは 200 で一覧を返す

#### Scenario: version エンドポイントのテストが存在する
- **GIVEN** server API テストスイートが実行される
- **WHEN** version エンドポイントのテストが含まれる
- **THEN** `GET /api/v1/version` が 200 を返すことが検証される
- **AND** レスポンスの `version` フィールドが空でないことが検証される
