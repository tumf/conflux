## Context
server の WebSocket は FullState の定期送信のみで、runner stdout/stderr や実行ログが配信されていない。リモート TUI では進捗は動くがログが見えず、実行が止まったように見える。

## Design
- server runner が stdout/stderr を LogEntry に変換し、共有ログキューへ送る
- WebSocket は FullState と Log イベントを同一接続で配信する
