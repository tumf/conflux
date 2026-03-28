# Change: ダッシュボードでWebSocket logメッセージをストアに反映する

## Why
サーバーモードのダッシュボードでは、WebSocket経由で `log` タイプのメッセージを受信しているが、現在の `wsClient.ts` はこれを無視している。そのため、ダッシュボードのLogsパネルにリアルタイムログが表示されない。

## What Changes
- `wsClient.ts` に `onLogEntry` リスナーを追加し、`log` メッセージ受信時にコールバックを呼ぶ
- `useWebSocket.ts` に `onLogEntry` オプションを追加
- `App.tsx` で `onLogEntry` を `store.appendLog` に接続
- `RemoteLogEntry` 型をサーバーのスキーマ（`message`, `level`, `change_id`, `timestamp`, `project_id`, `operation`, `iteration`）に合わせる

## Impact
- Affected specs: web-monitoring
- Affected code: `dashboard/src/api/wsClient.ts`, `dashboard/src/hooks/useWebSocket.ts`, `dashboard/src/App.tsx`, `dashboard/src/api/types.ts`
