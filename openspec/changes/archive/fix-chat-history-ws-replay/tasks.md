## Implementation Tasks

- [x] 1. `ProposalWsServerMessage` enum に `UserMessage` バリアントを追加（`src/server/api.rs`）。verification: `cargo build` が通り、`UserMessage { id, content, timestamp }` がシリアライズ可能
- [x] 2. WS接続時リプレイループで user メッセージを `user_message` として送信（`src/server/api.rs` L3503-3542 付近）。verification: WS接続時に user ロールのメッセージも `user_message` イベントとして送信される
- [x] 3. フロントの WSメッセージ型に `user_message` を追加（`dashboard/src/api/types.ts`）。verification: TypeScript の型定義に `user_message` が含まれる
- [x] 4. `useProposalWebSocket` の `handleServerMessage` で `user_message` をハンドリング（`dashboard/src/hooks/useProposalWebSocket.ts`）。verification: `onUserMessage` コールバックが呼ばれ、メッセージが state に追加される
- [x] 5. `ProposalChat.tsx` から REST hydrate の `useEffect`（`listProposalSessionMessages` 呼び出し）を削除。verification: `listProposalSessionMessages` がフロントコードから呼ばれない
- [x] 6. `ProposalChat.tsx` で `onUserMessage` コールバックを接続し、WS リプレイの user メッセージを `onAppendMessage` 経由で state に反映。verification: WS再接続後に user メッセージがチャット欄に表示される
- [x] 7. `useProposalWebSocket` の `onclose` ハンドラで、active turn がある場合に `onError` をコールバック。verification: WS切断後に `isAgentResponding` が `false` になり入力欄が有効になる
- [x] 8. テスト更新: `ProposalChat.test.tsx` から REST hydrate 関連のテストを削除し、WS リプレイ経由の復元テストに置換。verification: `cd dashboard && npm test` が通る
- [x] 9. Rust 側テスト: WS リプレイで user メッセージも送信されることを検証。verification: `cargo test` が通る
- [x] 10. `cargo fmt --check && cargo clippy -- -D warnings` が通る。verification: lint エラーなし

## Future Work

- REST API `listProposalSessionMessages` の廃止検討（現時点ではデバッグ用に残す）
