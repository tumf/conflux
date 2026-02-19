## ADDED Requirements
### Requirement: リモートTUI向けのログ配信
サーバは WebSocket の状態更新に、プロジェクト実行中のログを含めて配信しなければならない（MUST）。

ログは少なくとも以下を含む:
- `project_id`
- `change_id`（不明な場合は `null`）
- `operation`
- `iteration`
- `message`
- `level`
- `timestamp`

#### Scenario: 実行ログが WebSocket で配信される
- **GIVEN** サーバが `POST /api/v1/projects/{id}/control/run` を受け付けた
- **WHEN** 実行中に stdout/stderr が出力される
- **THEN** サーバは WebSocket でログイベントを配信する

#### Scenario: ログは変更一覧の更新と同時に到達する
- **GIVEN** リモートTUIが WebSocket を購読している
- **WHEN** ログイベントが配信される
- **THEN** TUI は該当 change のログプレビューを更新できる
