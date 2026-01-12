# Circuit Breaker Capability

## ADDED Requirements

### Requirement: APIレート制限ハンドリング

OrchestratorはエージェントAPIのレート制限エラーを検出し、設定された戦略に従って待機または停止しなければならない（SHALL）。

#### Scenario: レート制限エラー検出時に待機戦略を実行
- **GIVEN** config内で`rate_limit_handler.strategy = "wait"`が設定されている
- **AND** エージェントapply実行でレート制限エラーが発生する
- **AND** エラーメッセージに"rate limit exceeded, retry after 3600 seconds"が含まれる
- **WHEN** レート制限ハンドラーがエラーを処理する
- **THEN** 3600秒（60分）の待機が開始される
- **AND** カウントダウンタイマーが表示される
- **AND** 待機完了後、同じchangeで再試行される

#### Scenario: レート制限エラー検出時に終了戦略を実行
- **GIVEN** config内で`rate_limit_handler.strategy = "exit"`が設定されている
- **AND** エージェントapply実行でレート制限エラーが発生する
- **WHEN** レート制限ハンドラーがエラーを処理する
- **THEN** infoログで終了理由を出力する
- **AND** orchestratorが正常終了する

#### Scenario: レート制限エラー検出時にスキップ戦略を実行
- **GIVEN** config内で`rate_limit_handler.strategy = "skip"`が設定されている
- **AND** エージェントapply実行でレート制限エラーが発生する
- **WHEN** レート制限ハンドラーがエラーを処理する
- **THEN** 現在のchangeをスキップする
- **AND** 次のchangeへ移行する

#### Scenario: Retry-Afterヘッダーから待機時間を取得
- **GIVEN** レート制限エラーレスポンスに"Retry-After: 7200"ヘッダーが含まれる
- **WHEN** 待機時間を抽出する
- **THEN** 7200秒（2時間）の待機時間が設定される

#### Scenario: レート制限以外のエラーでは発動しない
- **GIVEN** エージェントapply実行で"Connection timeout"エラーが発生する
- **WHEN** レート制限ハンドラーがエラーをチェックする
- **THEN** レート制限エラーではないと判定される
- **AND** 通常のエラーハンドリングが実行される
