## MODIFIED Requirements
### Requirement: Streaming 対応リトライ

コマンドキューは streaming 出力を伴うコマンド実行でも、既存のリトライ判定ロジック（エラーパターン、実行時間、exit code）を適用しなければならない (MUST)。

Streaming リトライの動作は以下の通りとする：
- コマンド実行中、stdout/stderr を逐次出力チャネルに送信する
- stderr を同時にバッファリングしてリトライ判定に使用する
- コマンド失敗時、通常のリトライ判定ロジックを適用する
- リトライ時は出力チャネルにリトライ通知を送信する
- 新しいコマンドを spawn して再度 streaming を開始する

#### Scenario: Streaming 実行でリトライが適用される
- **GIVEN** streaming 実行経路でコマンドが失敗する
- **WHEN** exit code が 0 以外で終了する
- **THEN** 既存のリトライ判定ロジックが適用される
- **AND** リトライ通知が出力チャネルに送信される
