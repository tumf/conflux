## 1. Implementation
- [x] 1.1 Worktreesビューの削除判定を対象worktreeのchange状態で分岐する（verify: `src/tui/state/mod.rs` の `request_worktree_delete_from_list` がchange関連を判定している）
- [x] 1.2 worktreeブランチからchange_idを抽出して一覧と照合する（verify: `src/tui/state/mod.rs` で `GitWorkspaceManager::extract_change_id_from_worktree_name` を呼んでいる）
- [x] 1.3 queued/processing系ステータスの削除拒否メッセージを明確にする（verify: warningメッセージにchange_idと状態が含まれる）
- [x] 1.4 worktree削除関連テストを追加/更新する（verify: `cargo test` で該当テストが通る）

## 2. Validation
- [x] 2.1 `npx @fission-ai/openspec@latest validate allow-worktree-delete-while-running --strict` を実行し成功する
