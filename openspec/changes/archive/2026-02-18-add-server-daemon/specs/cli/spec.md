## ADDED Requirements
### Requirement: server サブコマンド
CLI は `cflx server` サブコマンドを提供し、サーバモードを起動しなければならない（SHALL）。

#### Scenario: server サブコマンドで起動する
- **WHEN** ユーザーが `cflx server` を実行する
- **THEN** サーバモードが起動する
- **AND** カレントディレクトリの変更一覧は読み込まない
