## MODIFIED Requirements

### Requirement: REST API - 変更の承認
HTTPサーバーは、変更を承認するためのREST APIエンドポイントを提供してはならない（SHALL NOT）。

#### Scenario: 承認エンドポイントは利用できない
- **WHEN** クライアントが `POST /api/changes/{id}/approve` を送信する
- **THEN** サーバーはHTTP 404またはメソッド未対応のエラーで応答する
- **AND** 承認状態は変更されない

### Requirement: REST API - 変更の承認解除
HTTPサーバーは、変更の承認を解除するためのREST APIエンドポイントを提供してはならない（SHALL NOT）。

#### Scenario: 承認解除エンドポイントは利用できない
- **WHEN** クライアントが `POST /api/changes/{id}/unapprove` を送信する
- **THEN** サーバーはHTTP 404またはメソッド未対応のエラーで応答する
- **AND** 承認状態は変更されない

### Requirement: 承認状態変更のWebSocket通知
HTTPサーバーは、承認状態変更の通知をWebSocketで配信してはならない（SHALL NOT）。

#### Scenario: 承認状態フィールドは配信されない
- **WHEN** サーバーが change の状態更新を送信する
- **THEN** メッセージには `is_approved` フィールドが含まれない

### Requirement: ダッシュボードUI - 承認ボタン
Webダッシュボードは、各変更カードに承認/承認解除ボタンを表示してはならない（SHALL NOT）。

#### Scenario: 承認ボタンが表示されない
- **WHEN** 変更カードがダッシュボードに表示される
- **THEN** 「Approve」「Unapprove」ボタンは表示されない
