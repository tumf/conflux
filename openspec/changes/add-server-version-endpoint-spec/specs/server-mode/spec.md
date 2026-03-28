## ADDED Requirements

### Requirement: Version エンドポイントを認証なしで提供する

サーバは `GET /api/v1/version` で現在のバージョン文字列を認証なしで返さなければならない（MUST）。レスポンスは JSON 形式で `version` フィールドを含む。

#### Scenario: version エンドポイントが認証なしで応答する

- **GIVEN** サーバが起動している
- **WHEN** クライアントが `GET /api/v1/version` を認証ヘッダーなしで呼び出す
- **THEN** サーバは 200 を返す
- **AND** レスポンスボディに `version` フィールドが含まれる
- **AND** `version` フィールドはセマンティックバージョンとビルド番号を含む文字列である

#### Scenario: version エンドポイントは bearer token が設定されていても認証不要である

- **GIVEN** サーバが bearer token 認証を有効にして起動している
- **WHEN** クライアントが認証ヘッダーなしで `GET /api/v1/version` を呼び出す
- **THEN** サーバは 200 を返す
