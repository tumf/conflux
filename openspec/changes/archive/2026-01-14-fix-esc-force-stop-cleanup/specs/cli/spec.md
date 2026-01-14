## MODIFIED Requirements
### Requirement: TUI Stop Processing with Escape Key
TUIはEsc二度押しによる強制停止時、現在のエージェントプロセスとその子プロセスを確実に終了しなければならない（SHALL）。

#### Scenario: 強制停止で子プロセスが残らない
- **WHEN** TUIがStoppingモードでユーザーがEscを再度押す
- **THEN** 現在のエージェントプロセスとその子プロセスが終了する
- **AND** 終了待機がタイムアウトした場合でも、追加の終了処理が行われる
- **AND** ログに「Force stopped - process terminated」が表示される
- **AND** 変更の状態はQueuedに戻る
