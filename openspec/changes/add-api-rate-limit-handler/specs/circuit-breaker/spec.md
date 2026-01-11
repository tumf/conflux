# Circuit Breaker Capability

## ADDED Requirements

### Requirement: API rate limit handling

The Orchestrator MUST detect agent API rate limit errors and either wait appropriately or stop execution.

#### Scenario: レート制限エラー検出時に待機戦略を実行

**Given** config内で`rate_limit_handler.strategy = "wait"`が設定されている  
**And** エージェントapply実行でレート制限エラーが発生  
**And** エラーメッセージに"rate limit exceeded, retry after 3600 seconds"が含まれる  
**When** レート制限ハンドラーがエラーを処理する  
**Then** 3600秒（60分）の待機が開始される  
**And** カウントダウンタイマーが表示される  
**And** 待機完了後、同じchangeで再試行される

#### Scenario: レート制限エラー検出時に終了戦略を実行

**Given** config内で`rate_limit_handler.strategy = "exit"`が設定されている  
**And** エージェントapply実行でレート制限エラーが発生  
**When** レート制限ハンドラーがエラーを処理する  
**Then** infoログで終了理由を出力する  
**And** orchestratorが正常終了する

#### Scenario: レート制限エラー検出時にスキップ戦略を実行

**Given** config内で`rate_limit_handler.strategy = "skip"`が設定されている  
**And** エージェントapply実行でレート制限エラーが発生  
**When** レート制限ハンドラーがエラーを処理する  
**Then** 現在のchangeをスキップする  
**And** 次のchangeへ移行する

#### Scenario: Retry-Afterヘッダーから待機時間を取得

**Given** レート制限エラーレスポンスに"Retry-After: 7200"ヘッダーが含まれる  
**When** 待機時間を抽出する  
**Then** 7200秒（2時間）の待機時間が設定される

#### Scenario: レート制限以外のエラーでは発動しない

**Given** エージェントapply実行で"Connection timeout"エラーが発生  
**When** レート制限ハンドラーがエラーをチェックする  
**Then** レート制限エラーではないと判定される  
**And** 通常のエラーハンドリングが実行される
