## ADDED Requirements
### Requirement: サーバの auto_resolve は共通 resolve_command を使用する
サーバは auto_resolve における解決コマンドとして、設定マージ済みのトップレベル `resolve_command` を使用しなければならない（MUST）。

#### Scenario: auto_resolve で共通 resolve_command が使われる
- **GIVEN** 設定のマージ結果に `resolve_command` が存在する
- **AND** `auto_resolve=true` が指定されている
- **WHEN** サーバが `git/pull` を処理する
- **THEN** サーバはトップレベル `resolve_command` を実行する
