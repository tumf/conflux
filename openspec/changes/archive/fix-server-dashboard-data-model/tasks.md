## Implementation Tasks

- [x] 1. `src/remote/types.rs`: `RemoteProject` に `repo: String`, `branch: String`, `status: String`, `is_busy: bool`, `error: Option<String>` フィールドを追加 (verification: `cargo build` が成功する)
- [x] 2. `src/remote/types.rs`: 既存テストの `make_remote_project` ヘルパーを新フィールドに対応させる (verification: `cargo test remote::types` が通る)
- [x] 3. `src/server/api.rs`: `build_remote_project_snapshot_async` で `ProjectEntry` から `repo` (URL末尾)、`branch`、`status`、`is_busy` を算出して `RemoteProject` に設定する (verification: `cargo test server` が通る)
- [x] 4. `dashboard/src/api/types.ts`: `RemoteProject` 型に `name`, `changes` フィールドを追加し、`RemoteChange` のフィールド名をサーバースキーマに合わせる (`project`, `completed_tasks`, `total_tasks`, `last_modified`, `iteration_number`) (verification: TypeScriptビルドが成功)
- [x] 5. `dashboard/src/api/wsClient.ts`: `full_state` メッセージ受信時に `projects` から `changes` をフラット化して `FullState` に設定し、`worktrees` も渡す (verification: TypeScriptビルドが成功)
- [x] 6. `dashboard/src/components/ChangesPanel.tsx`: フィルタ条件を `project_id` から `project` に変更する (verification: TypeScriptビルドが成功)
- [x] 7. `dashboard/src/store/useAppStore.ts`: `FullState` のフィールドがサーバーレスポンスと整合していることを確認 (verification: `dashboard/src/store/useAppStore.test.ts` が通る)
- [x] 8. `cargo fmt && cargo clippy -- -D warnings && cargo test` で全体検証
- [x] 9. `cd dashboard && npm run build` でダッシュボードビルド成功を確認
