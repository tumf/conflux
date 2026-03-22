## MODIFIED Requirements

### Requirement: リモートサーバ指定フラグ
CLI は `--server <endpoint>` を受け付け、TUI をリモートサーバ接続モードで起動しなければならない（SHALL）。

#### Scenario: リモートエンドポイントで TUI を起動する
- **WHEN** ユーザーが `cflx --server http://127.0.0.1:39876` を実行する
- **THEN** TUI はローカルの change 一覧を読まずにリモート状態を表示する

### Requirement: Project サブコマンドの接続先解決と認証非対応
`cflx project` は `--server` 未指定時にグローバル設定の `server.bind` と `server.port` を用いて接続先を決定しなければならない（MUST）。
今回の `cflx project` はサーバクライアント認証を扱わず、認証が必要な設定が指定された場合は実行前にエラーで停止しなければならない（MUST）。

#### Scenario: --server 未指定時はグローバル設定から接続先を解決する
- **GIVEN** グローバル設定に `server.bind` と `server.port` がある
- **WHEN** ユーザーが `cflx project status` を実行する
- **THEN** CLI は `http://<bind>:<port>` を接続先として使用する

#### Scenario: --server 未指定時は既定値から接続先を解決する
- **GIVEN** グローバル設定に `server.bind` と `server.port` がない
- **WHEN** ユーザーが `cflx project status` を実行する
- **THEN** CLI は `http://127.0.0.1:39876` を接続先として使用する

#### Scenario: 認証指定がある場合はエラーで停止する
- **GIVEN** `--server-token` または `--server-token-env` が指定されている
- **WHEN** ユーザーが `cflx project` サブコマンドを実行する
- **THEN** CLI は認証非対応である旨を示すエラーを表示し、リクエストを送信しない
