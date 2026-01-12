## MODIFIED Requirements

### Requirement: WebSocket - Real-time Updates
HTTPサーバーは、WebSocket接続を通じてリアルタイムの状態更新をサポートしなければならない（SHALL）。

**変更内容**: TUIモードでも状態更新がブロードキャストされるよう、TUIオーケストレーターがWebStateへの参照を持ち、状態変更時にupdate()を呼び出すことを明確化。

#### Scenario: WebSocket connection established
- **WHEN** クライアントが`ws://localhost:8080/ws`に接続する
- **THEN** サーバーはWebSocketアップグレードを受け入れる
- **AND** 双方向通信のために接続が維持される

#### Scenario: State update broadcast
- **WHEN** オーケストレーターの状態が変化する（タスク完了、新しい変更など）
- **THEN** サーバーはすべての接続されたWebSocketクライアントにJSONメッセージをブロードキャストする
- **AND** メッセージにはタイムスタンプと更新された変更データが含まれる
- **AND** メッセージ形式は`{"type": "state_update", "timestamp": "...", "changes": [...]}`である

#### Scenario: TUI mode state updates
- **WHEN** TUIモードで`--web`オプションを使用して起動する
- **THEN** WebStateがTUIオーケストレーターに渡される
- **AND** オーケストレーターのループ内で状態変更時にWebStateのupdate()が呼び出される
- **AND** WebSocketクライアントに状態更新がブロードキャストされる

#### Scenario: Multiple concurrent clients
- **WHEN** 複数のクライアントが同時にWebSocket経由で接続する
- **THEN** すべてのクライアントが状態更新ブロードキャストを受信する
- **AND** 各クライアントは独立した接続を維持する
- **AND** 1つのクライアントの切断は他のクライアントに影響しない

#### Scenario: WebSocket client disconnection
- **WHEN** クライアントがWebSocket接続を閉じる
- **THEN** サーバーは接続リソースをクリーンアップする
- **AND** サーバーは残りのクライアントへのブロードキャストを継続する
