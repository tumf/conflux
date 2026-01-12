## ADDED Requirements

### Requirement: REST API - 変更の承認
HTTPサーバーは、変更を承認するためのREST APIエンドポイントを提供しなければならない（SHALL）。

#### Scenario: 変更の承認成功
- **WHEN** クライアントが `POST /api/changes/{id}/approve` を送信する
- **AND** 指定されたIDの変更が存在する
- **THEN** サーバーはHTTP 200ステータスで応答する
- **AND** レスポンスボディには `{"success": true, "is_approved": true}` を含む
- **AND** 変更の`approved`ファイルが作成される

#### Scenario: 存在しない変更の承認
- **WHEN** クライアントが `POST /api/changes/nonexistent/approve` を送信する
- **AND** 指定されたIDの変更が存在しない
- **THEN** サーバーはHTTP 404ステータスで応答する
- **AND** レスポンスボディにはエラーメッセージを含む

#### Scenario: 既に承認済みの変更を承認
- **WHEN** クライアントが既に承認済みの変更に `POST /api/changes/{id}/approve` を送信する
- **THEN** サーバーはHTTP 200ステータスで応答する
- **AND** 承認状態は維持される（冪等操作）

### Requirement: REST API - 変更の承認解除
HTTPサーバーは、変更の承認を解除するためのREST APIエンドポイントを提供しなければならない（SHALL）。

#### Scenario: 変更の承認解除成功
- **WHEN** クライアントが `POST /api/changes/{id}/unapprove` を送信する
- **AND** 指定されたIDの変更が存在する
- **THEN** サーバーはHTTP 200ステータスで応答する
- **AND** レスポンスボディには `{"success": true, "is_approved": false}` を含む
- **AND** 変更の`approved`ファイルが削除される

#### Scenario: 存在しない変更の承認解除
- **WHEN** クライアントが `POST /api/changes/nonexistent/unapprove` を送信する
- **AND** 指定されたIDの変更が存在しない
- **THEN** サーバーはHTTP 404ステータスで応答する
- **AND** レスポンスボディにはエラーメッセージを含む

#### Scenario: 未承認の変更を承認解除
- **WHEN** クライアントが未承認の変更に `POST /api/changes/{id}/unapprove` を送信する
- **THEN** サーバーはHTTP 200ステータスで応答する
- **AND** 承認状態は維持される（冪等操作）

### Requirement: 承認状態変更のWebSocket通知
HTTPサーバーは、承認状態が変更されたときにWebSocket経由で全クライアントに通知しなければならない（SHALL）。

#### Scenario: 承認時の通知
- **WHEN** REST API経由で変更が承認される
- **THEN** WebSocketで接続中の全クライアントに状態更新メッセージが送信される
- **AND** メッセージには更新された変更の`is_approved`フィールドが含まれる

#### Scenario: 承認解除時の通知
- **WHEN** REST API経由で変更の承認が解除される
- **THEN** WebSocketで接続中の全クライアントに状態更新メッセージが送信される
- **AND** メッセージには更新された変更の`is_approved`フィールドが含まれる

### Requirement: ダッシュボードUI - 承認ボタン
Webダッシュボードは、各変更カードに承認/承認解除のトグルボタンを表示しなければならない（SHALL）。

#### Scenario: 未承認変更の承認ボタン表示
- **WHEN** 未承認の変更がダッシュボードに表示される
- **THEN** 変更カードに「Approve」ボタンが表示される
- **AND** ボタンはオレンジまたは警告色で表示される

#### Scenario: 承認済み変更の承認解除ボタン表示
- **WHEN** 承認済みの変更がダッシュボードに表示される
- **THEN** 変更カードに「Unapprove」ボタンが表示される
- **AND** ボタンは緑または成功色で表示される

#### Scenario: 承認ボタンのクリック
- **WHEN** ユーザーが承認ボタンをクリックする
- **THEN** REST APIに承認リクエストが送信される
- **AND** 成功時にボタンの状態が即座に更新される
- **AND** エラー時にトースト通知でエラーメッセージが表示される

#### Scenario: 承認ボタンのタッチ対応
- **WHEN** 承認ボタンがモバイルデバイスに表示される
- **THEN** ボタンのタップ可能領域は最小44x44ピクセルである
- **AND** タップ時に視覚的フィードバックが表示される
