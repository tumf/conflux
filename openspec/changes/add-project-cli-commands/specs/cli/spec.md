## ADDED Requirements

### Requirement: Project サブコマンドによるサーバプロジェクト管理
CLI は `cflx project` 配下でサーバのプロジェクト管理 API を操作できなければならない（SHALL）。
`add` は `remote_url` と `branch` を送信し、`remove` は `project_id` を削除し、`status` はプロジェクト一覧または指定 ID の情報を取得し、`sync` は git/sync を実行しなければならない（SHALL）。

#### Scenario: プロジェクト追加を実行する
- **WHEN** ユーザーが `cflx project add <remote_url> <branch>` を実行する
- **THEN** CLI は `POST /api/v1/projects` を呼び出す
- **AND** サーバが返す `project_id` を表示する

#### Scenario: プロジェクト一覧を取得する
- **WHEN** ユーザーが `cflx project status` を実行する
- **THEN** CLI は `GET /api/v1/projects` を呼び出す
- **AND** 取得したプロジェクト一覧を表示する

#### Scenario: プロジェクトの同期を実行する
- **WHEN** ユーザーが `cflx project sync <project_id>` を実行する
- **THEN** CLI は `POST /api/v1/projects/{id}/git/sync` を呼び出す
- **AND** サーバの同期結果を表示する

### Requirement: Project サブコマンドの接続先解決と認証非対応
`cflx project` は `--server` 未指定時にグローバル設定の `server.bind` と `server.port` を用いて接続先を決定しなければならない（MUST）。
今回の `cflx project` はサーバクライアント認証を扱わず、認証が必要な設定が指定された場合は実行前にエラーで停止しなければならない（MUST）。

#### Scenario: --server 未指定時はグローバル設定から接続先を解決する
- **GIVEN** グローバル設定に `server.bind` と `server.port` がある
- **WHEN** ユーザーが `cflx project status` を実行する
- **THEN** CLI は `http://<bind>:<port>` を接続先として使用する

#### Scenario: 認証指定がある場合はエラーで停止する
- **GIVEN** `--server-token` または `--server-token-env` が指定されている
- **WHEN** ユーザーが `cflx project` サブコマンドを実行する
- **THEN** CLI は認証非対応である旨を示すエラーを表示し、リクエストを送信しない
