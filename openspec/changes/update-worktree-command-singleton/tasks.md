## Implementation Tasks

- [ ] 1. worktree root（base / worktree）を一意に識別できる active command レジストリを server-mode に追加する（verification: `src/server/mod.rs` と関連 server 状態管理コードに active command 管理が追加されている）
- [ ] 2. `git/sync` が対象 base root の active command を登録し、busy 時は待機せず `409 Conflict` を返すよう更新する（verification: `src/server/api.rs` の sync ハンドラと API テストで 409 応答が確認できる）
- [ ] 3. apply / merge / worktree delete など worktree root を変更しうる server 操作を同じ active command レジストリに統合する（verification: 対象 API ハンドラが同一 root busy 時に 409 を返す）
- [ ] 4. WebSocket `full_state` と REST 状態レスポンスに active command 情報を含める（verification: `src/server/api.rs` の full_state payload と `dashboard/src/api/types.ts` が一致している）
- [ ] 5. dashboard の project/worktree 操作 UI を active command 状態で disable し、Syncing などの進行表示をリロード後も復元する（verification: `dashboard/src/App.tsx`, `dashboard/src/store/useAppStore.ts`, `dashboard/src/components/ProjectCard.tsx`, `dashboard/src/components/WorktreeRow.tsx`）
- [ ] 6. `git/sync` の resolve_command stdout/stderr と開始/完了/失敗イベントをプロジェクトログへ流す（verification: `src/server/api.rs` のログ配信処理と対応テストで `project_id` 付きログが確認できる）
- [ ] 7. server-mode / dashboard 向け回帰テストを追加し、busy root の 409・full_state 復元・sync ログ表示を検証する（verification: `cargo test` で関連テストが通る）

## Future Work

- サーバー再起動後の active command 再構築
- root 単位排他に加えた project 単位の弱い整合制約の導入検討
