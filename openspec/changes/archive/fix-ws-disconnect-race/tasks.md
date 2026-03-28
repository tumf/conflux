## Implementation Tasks

- [x] 1. `wsClient.ts` に `connectAborted` フラグを追加し、`connect()` の `onopen` で abort チェックを行う (verification: `dashboard/src/api/wsClient.ts` に `connectAborted` プロパティが存在する)
- [x] 2. `disconnect()` で `readyState` に応じた分岐を実装: `CONNECTING` → `onopen` で即 `close()`、`OPEN` → 即 `close()`、それ以外 → no-op (verification: `disconnect()` メソッド内に `WebSocket.CONNECTING` チェックが存在する)
- [x] 3. `connect()` の `onopen` コールバックで `connectAborted` が true なら `close()` して早期 return する (verification: `onopen` 内に `this.connectAborted` チェックが存在する)
- [x] 4. ビルド確認 (verification: `cd dashboard && npm run build` が成功する)
