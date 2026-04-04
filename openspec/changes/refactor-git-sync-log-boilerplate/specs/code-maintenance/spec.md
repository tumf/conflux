## ADDED Requirements

### Requirement: resolve_command ログの一貫した生成
システムは `resolve_command` 実行時のサーバーログについて、開始・標準出力・標準エラー・終了を既存どおりの意味と順序で記録しなければならない。

#### Scenario: resolve_command 成功時にログ順序が維持される
- **GIVEN** `resolve_command` が正常終了し、標準出力と標準エラーに1行ずつ出力する
- **WHEN** サーバーがログを収集する
- **THEN** 開始ログが最初に記録される
- **AND** 標準出力は `info`、標準エラーは `warn` として記録される
- **AND** 終了ログが最後に記録される

#### Scenario: リファクタ後も API 公開挙動は変わらない
- **GIVEN** `git/sync` が `resolve_command` を必要とする既存構成で実行される
- **WHEN** 同期処理を呼び出す
- **THEN** HTTPレスポンス形式とログの意味は変更されない
