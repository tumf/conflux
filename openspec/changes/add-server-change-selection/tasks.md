## Implementation Tasks

- [ ] Task 1: `RemoteChange` に `selected: bool` フィールドを追加 (`src/remote/types.rs`) (verification: `cargo build` が通る、`RemoteChange` の JSON に `selected` が含まれる)
- [ ] Task 2: `ProjectRegistry` に per-project per-change の selected 状態を保持する HashMap を追加 (`src/server/registry.rs`) (verification: `cargo test` が通る)
- [ ] Task 3: change 同期時に新規 change を `selected: true` で初期化するロジックを追加 (`src/server/registry.rs` or `src/server/api.rs`) (verification: プロジェクト追加後に全 change が selected で返る)
- [ ] Task 4: `POST /api/v1/projects/{id}/changes/{change_id}/toggle` エンドポイントを追加 (`src/server/api.rs`) (verification: curl で toggle して WebSocket の change_update に反映される)
- [ ] Task 5: `POST /api/v1/projects/{id}/changes/toggle-all` エンドポイントを追加 (`src/server/api.rs`) (verification: curl で toggle-all して全 change の selected が反転する)
- [ ] Task 6: WebSocket `full_state` 生成時に selected 状態を `RemoteChange` に反映 (`src/server/api.rs`) (verification: WebSocket メッセージに `selected` フィールドが含まれる)
- [ ] Task 7: ダッシュボード TypeScript 型に `selected: boolean` を追加 (`dashboard/src/api/types.ts`) (verification: TypeScript コンパイルが通る)
- [ ] Task 8: `restClient.ts` に `toggleChangeSelection` と `toggleAllChangeSelection` API 関数を追加 (`dashboard/src/api/restClient.ts`) (verification: TypeScript コンパイルが通る)
- [ ] Task 9: `ChangeRow` コンポーネントにチェックボックスを追加し、クリックで toggle API を呼ぶ (`dashboard/src/components/ChangeRow.tsx`) (verification: `cd dashboard && npm run build` が通る)
- [ ] Task 10: `cargo clippy -- -D warnings` と `cargo fmt --check` が通ることを確認 (verification: CI lint と同等)
- [ ] Task 11: `cd dashboard && npm run build` が通ることを確認 (verification: ダッシュボードビルド成功)
