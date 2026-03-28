## Implementation Tasks

- [x] 1. `dashboard/src/api/types.ts`: `RemoteLogEntry` 型をサーバースキーマに合わせる（`message`, `level`, `change_id`, `timestamp`, `project_id`, `operation`, `iteration`） (verification: TypeScriptビルド成功)
- [x] 2. `dashboard/src/api/wsClient.ts`: `onLogEntry` リスナーを追加し、`message.type === 'log'` の場合に `message.entry` をコールバックで通知する (verification: TypeScriptビルド成功)
- [x] 3. `dashboard/src/hooks/useWebSocket.ts`: `UseWebSocketOptions` に `onLogEntry` を追加し、wsClientのリスナーに登録する (verification: TypeScriptビルド成功)
- [x] 4. `dashboard/src/App.tsx`: `useWebSocket` の `onLogEntry` に `store.appendLog` を接続する (verification: TypeScriptビルド成功)
- [x] 5. `dashboard/src/store/useAppStore.ts`: `APPEND_LOG` アクションが新しい `RemoteLogEntry` スキーマで動作することを確認。`project_id` でログをグループ化する既存ロジックがサーバーの `project_id` フィールドと整合していることを検証 (verification: `useAppStore.test.ts` パス)
- [x] 6. `cd dashboard && npm run build` でビルド成功を確認
