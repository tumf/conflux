## Implementation Tasks

- [ ] 1. `src/server/api.rs`: `AppState` に `sync_available: bool` を追加し、`resolve_command.is_some()` で設定する (verification: `cargo build`)
- [ ] 2. `src/server/api.rs`: `projects_state` レスポンスに `sync_available` を含める (verification: `cargo test server`)
- [ ] 3. `src/server/api.rs`: WebSocket `full_state` メッセージにトップレベル `sync_available` フィールドを追加する (verification: `cargo test server`)
- [ ] 4. `dashboard/src/api/types.ts`: `FullState` に `sync_available?: boolean` を追加 (verification: TypeScriptビルド成功)
- [ ] 5. `dashboard/src/store/useAppStore.ts`: `AppState` に `syncAvailable: boolean` を追加し、`SET_FULL_STATE` で更新する (verification: テスト通過)
- [ ] 6. `dashboard/src/components/ProjectCard.tsx`: `syncAvailable` が `false` の場合、Syncボタンを `disabled` にしツールチップで理由を表示する (verification: TypeScriptビルド成功)
- [ ] 7. `cargo fmt && cargo clippy -- -D warnings && cargo test` で全体検証
- [ ] 8. `cd dashboard && npm run build` でビルド成功を確認
