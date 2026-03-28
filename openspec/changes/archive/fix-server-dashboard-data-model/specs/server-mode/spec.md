## MODIFIED Requirements

### Requirement: API v1 を提供する
サーバは `api/v1` 名前空間でプロジェクト管理と実行制御の API を提供しなければならない（SHALL）。

WebSocket の `full_state` メッセージに含まれる各 `RemoteProject` は、ダッシュボード互換のために以下のフィールドを含まなければならない（MUST）:
- `id`: プロジェクト識別子
- `name`: 表示名 ("repo@branch" 形式)
- `repo`: リポジトリ名（remote_url の末尾から抽出）
- `branch`: ブランチ名
- `status`: 実行状態 ("idle" | "running" | "stopped")
- `is_busy`: 実行中かどうか (status == "running")
- `error`: エラーメッセージ（なければ null）
- `changes`: プロジェクトに属する変更の配列

#### Scenario: プロジェクト管理 API が応答する
- **WHEN** クライアントが `GET /api/v1/projects` を呼び出す
- **THEN** サーバは 200 で一覧を返す

#### Scenario: WebSocket full_state にダッシュボード互換フィールドが含まれる
- **GIVEN** ダッシュボードが WebSocket を購読している
- **WHEN** サーバーが `full_state` メッセージを送信する
- **THEN** 各プロジェクトに `repo`, `branch`, `status`, `is_busy`, `error` フィールドが含まれる
- **AND** `changes` 配列が各プロジェクトにネストされている
