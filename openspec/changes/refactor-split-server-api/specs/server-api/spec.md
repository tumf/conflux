## MODIFIED Requirements

### Requirement: global-orchestration-status

サーバーはグローバルなオーケストレーション状態 (Idle/Running/Stopped) を管理する。

モジュール分割後も同一のエンドポイントパスとレスポンス形式を維持しなければならない (SHALL)。

#### Scenario: リファクタリング後もエンドポイントが維持される

- **GIVEN** サーバー API が `src/server/api/` ディレクトリ構成に分割済みである
- **WHEN** `GET /api/v1/control/status` を呼び出す
- **THEN** 分割前と同一の JSON レスポンスが返される
