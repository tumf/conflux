## MODIFIED Requirements
### Requirement: サーバ設定セクション
設定ファイルはサーバモードのための `server` セクションを提供しなければならない（MUST）。

`server` セクションは最低限以下のキーを受け付ける:
- `bind`（既定: `127.0.0.1`）
- `port`（既定: `9876`）
- `max_concurrent_total`
- `auth.mode`（`none` または `bearer_token`）
- `auth.token` または `auth.token_env`
- `data_dir`

`server.resolve_command` は受け付けてはならない（MUST NOT）。サーバの auto_resolve で使用する resolve_command はトップレベルの `resolve_command` を使用しなければならない（MUST）。

#### Scenario: グローバル設定から server を読み込む
- **GIVEN** `~/.config/cflx/config.jsonc` に `server` セクションがある
- **WHEN** `cflx server` を起動する
- **THEN** サーバは `server` セクションの設定を使用する

#### Scenario: server.resolve_command は設定エラーになる
- **GIVEN** 設定ファイルに `server.resolve_command` が含まれている
- **WHEN** 設定を読み込む
- **THEN** 設定エラーとして失敗する
- **AND** エラーメッセージに `server.resolve_command` が含まれる

#### Scenario: サーバの auto_resolve はトップレベル resolve_command を使う
- **GIVEN** 設定のマージ結果にトップレベルの `resolve_command` が存在する
- **WHEN** サーバが auto_resolve を実行する
- **THEN** `resolve_command` が使用される
