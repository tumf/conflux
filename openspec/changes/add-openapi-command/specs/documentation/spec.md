## MODIFIED Requirements

### Requirement: OpenAPI YAML generation

ドキュメントは Web 監視 API の OpenAPI 3.1 YAML をコードから動的に生成し、`cflx openapi` および `GET /api/v2/openapi.yaml` で提供しなければならない（SHALL）。リポジトリは生成済み OpenAPI YAML/JSON を追跡してはならない（MUST NOT）。

#### Scenario: CLIでスキーマを出力する

- **WHEN** 開発者が `cflx openapi` を実行する
- **THEN** 標準出力に最新の `/api/v2` OpenAPI YAML が出力される
- **AND** 出力はファイルへリダイレクトして利用できる

#### Scenario: ライブAPIからスキーマを取得する

- **WHEN** クライアントが `GET /api/v2/openapi.yaml` を要求する
- **THEN** `cflx openapi` と同じ生成元の最新仕様が返される
- **AND** 生成済みスキーマファイルをリポジトリへ保存する必要がない

#### Scenario: 生成契約を検証する

- **WHEN** API実装または公開スキーマが変更される
- **THEN** repository-local contract tests は生成ドキュメントのルート・スキーマ・セキュリティ宣言を検証する
- **AND** CLI出力とライブAPI出力の不一致を検知して失敗する
