# Change: WebSocket disconnect時のレースコンディション修正

## Why
`wsClient.ts` の `disconnect()` が `CONNECTING` 状態の WebSocket に対して `close()` を呼ぶと、ブラウザが "WebSocket is closed before the connection is established" 警告を出す。React 18+ Strict Mode の二重マウント/アンマウントで頻発する。

## What Changes
- `disconnect()` で `readyState` をチェックし、`CONNECTING` 状態なら `onopen` で即閉じるパターンに変更
- `connect()` の Promise が `disconnect()` 後に resolve/reject されないよう abort フラグを追加
- `CLOSING`/`CLOSED` 状態では `close()` を呼ばない

## Impact
- Affected specs: web-monitoring
- Affected code: `dashboard/src/api/wsClient.ts`
