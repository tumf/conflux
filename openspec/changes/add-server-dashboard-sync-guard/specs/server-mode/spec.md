## ADDED Requirements

### Requirement: sync_available フラグの提供
サーバーは `projects/state` レスポンスおよび WebSocket `full_state` メッセージで `sync_available` フラグを提供しなければならない（MUST）。このフラグは `resolve_command` が設定されているかどうかを示す。

#### Scenario: resolve_command 設定済みの場合
- **GIVEN** サーバーの設定に `resolve_command` が存在する
- **WHEN** クライアントが `GET /api/v1/projects/state` を呼び出す
- **THEN** レスポンスに `sync_available: true` が含まれる

#### Scenario: resolve_command 未設定の場合
- **GIVEN** サーバーの設定に `resolve_command` が存在しない
- **WHEN** クライアントが `GET /api/v1/projects/state` を呼び出す
- **THEN** レスポンスに `sync_available: false` が含まれる

#### Scenario: WebSocket full_state に sync_available が含まれる
- **GIVEN** ダッシュボードが WebSocket を購読している
- **WHEN** サーバーが `full_state` メッセージを送信する
- **THEN** メッセージに `sync_available` フラグが含まれる
