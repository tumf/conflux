## ADDED Requirements
### Requirement: サーバ設定セクション
設定ファイルはサーバモードのための `server` セクションを提供しなければならない（MUST）。

`server` セクションは最低限以下のキーを受け付ける:
- `bind`（既定: `127.0.0.1`）
- `port`（既定: `9876`）
- `max_concurrent_total`
- `auth.mode`（`none` または `bearer_token`）
- `auth.token` または `auth.token_env`
- `data_dir`

#### Scenario: グローバル設定から server を読み込む
- **GIVEN** `~/.config/cflx/config.jsonc` に `server` セクションがある
- **WHEN** `cflx server` を起動する
- **THEN** サーバは `server` セクションの設定を使用する

#### Scenario: 非ループバック bind は bearer token 必須
- **GIVEN** `server.bind` がループバック以外である
- **AND** `server.auth.mode=none`
- **WHEN** 設定を読み込む
- **THEN** 設定エラーとして失敗する

#### Scenario: port 未指定は既定値を使う
- **GIVEN** `server.port` が未指定である
- **WHEN** 設定を読み込む
- **THEN** `server.port=9876` が使用される
