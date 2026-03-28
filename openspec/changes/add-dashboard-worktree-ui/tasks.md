## Implementation Tasks

- [ ] 1. `dashboard/src/api/types.ts` に `WorktreeInfo` 型（`path`, `label`, `head`, `branch`, `is_detached`, `is_main`, `is_merging`, `has_commits_ahead`, `merge_conflict`）と `MergeConflictInfo` 型を追加し、`FullState` に `worktrees: Record<string, WorktreeInfo[]>` を追加する (verification: TypeScriptコンパイルが通る)
- [ ] 2. `dashboard/src/api/restClient.ts` に `listWorktrees(projectId)`, `createWorktree(projectId, req)`, `deleteWorktree(projectId, branch)`, `mergeWorktree(projectId, branch)`, `refreshWorktrees(projectId)` 関数を追加する (verification: TypeScriptコンパイルが通る)
- [ ] 3. `dashboard/src/store/useAppStore.ts` の `AppState` に `worktreesByProjectId: Record<string, WorktreeInfo[]>` を追加し、`SET_FULL_STATE` アクションで `worktrees` を更新する (verification: TypeScriptコンパイルが通る)
- [ ] 4. `WorktreeRow.tsx` コンポーネントを作成する。ブランチ名、ラベル、状態バッジ（[MAIN], [DETACHED], [merging], [merged]）、コンフリクトバッジ（赤、ファイル数表示）、条件付きマージ/削除ボタンを表示する (verification: `npm run build` が成功する)
- [ ] 5. `WorktreesPanel.tsx` コンポーネントを作成する。選択プロジェクトのWorktree一覧、作成ボタン（+）、リフレッシュボタン、空状態表示を含む (verification: `npm run build` が成功する)
- [ ] 6. `CreateWorktreeDialog.tsx` を作成する。change_id入力フィールド、作成/キャンセルボタン、Radix AlertDialog を使用（既存 DeleteDialog パターンに従う）(verification: `npm run build` が成功する)
- [ ] 7. `DeleteWorktreeDialog.tsx` を作成する。確認メッセージとブランチ名表示、既存 DeleteDialog パターン踏襲 (verification: `npm run build` が成功する)
- [ ] 8. `App.tsx` のレイアウトを変更する。デスクトップ: 中央パネルにChanges/Worktreesタブ切り替え追加、モバイル: 下部タブに `worktrees` 追加（Projects | Changes | Worktrees | Logs）(verification: `npm run build` が成功する + ブラウザで表示確認)
- [ ] 9. マージ・削除操作の成功/失敗時に `sonner` の `toast` でトースト通知を表示する (verification: `npm run build` が成功する)
- [ ] 10. `npm run lint && npm run build` を通す (verification: コマンドが成功する)

## Future Work

- Worktree内でのコマンド実行UI
- Worktree作成時のchangeリストからの選択UI（サジェスト）
