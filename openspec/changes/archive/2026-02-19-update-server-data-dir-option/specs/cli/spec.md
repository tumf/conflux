## ADDED Requirements
### Requirement: server データディレクトリの CLI 上書き
CLI は `cflx server --data-dir <PATH>` を受け付け、サーバの `data_dir` を指定パスに上書きしなければならない（SHALL）。
`--data-dir` が未指定の場合、サーバはグローバル設定の `server.data_dir` または既定値を使用しなければならない（SHALL）。

#### Scenario: `--data-dir` を指定して起動する
- **WHEN** ユーザーが `cflx server --data-dir /var/lib/cflx` を実行する
- **THEN** サーバは `data_dir=/var/lib/cflx` を使用する

#### Scenario: `--data-dir` 未指定で起動する
- **GIVEN** グローバル設定に `server.data_dir=/tmp/cflx-server` がある
- **WHEN** ユーザーが `cflx server` を実行する
- **THEN** サーバは `data_dir=/tmp/cflx-server` を使用する
