## MODIFIED Requirements
### Requirement: WebSocket - Real-time Updates
HTTPサーバーは、TUIとWeb UIの状態が常に一致するように、TUIで発生する状態更新をWebSocket経由でリアルタイムにブロードキャストしなければならない（SHALL）。

この配信は変更一覧の進捗だけでなく、TUIで表示される状態（キュー状態、ログ、ワークツリー、実行中の操作など）を含む統一状態モデルに基づかなければならない（MUST）。

#### Scenario: TUIとWeb UIが完全に一致する更新が配信される
- **GIVEN** ユーザーが `tui --web` でオーケストレーターを起動している
- **AND** Web UI が `/ws` へ WebSocket 接続済みである
- **WHEN** TUI の状態が更新される（変更一覧、キュー、ログ、ワークツリー、実行中操作のいずれか）
- **THEN** サーバーは統一状態モデルに基づく `state_update` メッセージをブロードキャストする
- **AND** Web UI の表示は TUI と同じ内容・同じ更新タイミングで反映される
