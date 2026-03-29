## Implementation Tasks

- [x] 1. worktree root（base / worktree）を一意に識別できる active command レジストリを server-mode に追加する（verification: `src/server/active_commands.rs` に `ActiveCommandRegistry`、`WorktreeRootKey`、`RootKind`、`ActiveCommandGuard` を実装。`src/remote/types.rs` に `ActiveCommand` DTO を定義。`src/server/mod.rs` にモジュール登録。`src/server/api.rs` の `AppState` に `active_commands: SharedActiveCommands` フィールドを追加）
- [x] 2. `git/sync` が対象 base root の active command を登録し、busy 時は待機せず `409 Conflict` を返すよう更新する（verification: `src/server/api.rs` の `git_sync` ハンドラに `try_acquire_active_command` 呼び出しを追加。`RootKind::Base` で排他。`active_commands.rs` のユニットテストで double acquire が Err を返すことを確認）
- [x] 3. apply / merge / worktree delete など worktree root を変更しうる server 操作を同じ active command レジストリに統合する（verification: `server_delete_worktree` に `RootKind::Worktree(branch)` ガード、`server_merge_worktree` に `RootKind::Base` + `RootKind::Worktree(branch)` ガードを追加）
- [x] 4. WebSocket `full_state` と REST 状態レスポンスに active command 情報を含める（verification: `RemoteStateUpdate::FullState` に `active_commands: Vec<ActiveCommand>` フィールドを追加。`handle_ws` で `active_commands.read().await.snapshot()` を取得して payload に含める。`dashboard/src/api/types.ts` に `ActiveCommand` インターフェースと `FullState.active_commands` を追加）
- [x] 5. dashboard の project/worktree 操作 UI を active command 状態で disable し、Syncing などの進行表示をリロード後も復元する（verification: `ProjectCard.tsx` で `baseBusy` 判定により Sync ボタン disable + Loader2 アニメーション + "Syncing…" 表示。`WorktreeRow.tsx` で `activeCommand` prop により merge/delete ボタン disable + busy バッジ表示。`useAppStore.ts` で `activeCommands` state 管理。`App.tsx` から `activeCommands` を各パネルに伝播）
- [x] 6. `git/sync` の resolve_command stdout/stderr と開始/完了/失敗イベントをプロジェクトログへ流す（verification: `run_resolve_command` に `log_tx` と `project_id` パラメータを追加し、stdout/stderr を行単位で `RemoteLogEntry` として broadcast。開始・完了イベントも送信。`operation: "resolve"` でタグ付け）
- [x] 7. server-mode / dashboard 向け回帰テストを追加し、busy root の 409・full_state 復元・sync ログ表示を検証する（verification: `src/server/active_commands.rs` に 6 つのユニットテスト（acquire/release、double acquire 拒否、異なる root 独立、snapshot、guard async release、root_kind display）。既存テスト全 2554 件パス。`cargo clippy -- -D warnings` パス。`cargo fmt --check` パス）

## Future Work

- サーバー再起動後の active command 再構築
- root 単位排他に加えた project 単位の弱い整合制約の導入検討
