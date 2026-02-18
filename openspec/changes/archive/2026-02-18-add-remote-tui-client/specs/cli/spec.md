## ADDED Requirements
### Requirement: リモートサーバ指定フラグ
CLI は `--server <endpoint>` を受け付け、TUI をリモートサーバ接続モードで起動しなければならない（SHALL）。

#### Scenario: リモートエンドポイントで TUI を起動する
- **WHEN** ユーザーが `cflx --server http://127.0.0.1:9876` を実行する
- **THEN** TUI はローカルの change 一覧を読まずにリモート状態を表示する

### Requirement: リモートサーバ認証トークン
CLI は bearer token を指定するための `--server-token` または `--server-token-env` を受け付けなければならない（SHALL）。

#### Scenario: bearer token を付与して接続する
- **GIVEN** `--server-token` が指定されている
- **WHEN** TUI がリモートサーバへ接続する
- **THEN** Authorization header に bearer token が付与される
