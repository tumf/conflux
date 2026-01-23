## 1. Implementation
- [ ] 1.1 worktree の archive 判定を、コミットメッセージではなくファイル状態（clean 状態、`openspec/changes/<change_id>` 不在、archive エントリ存在）で判定できるよう更新する（`src/execution/archive.rs`, `src/execution/state.rs`）。確認方法: `detect_workspace_state` が更新後の判定ヘルパーを参照していることを確認する。
- [ ] 1.2 `WorkspaceState::Archived` で再開した場合に acceptance/archiving を再実行せず merge へ進むことを保証する（`src/parallel/mod.rs`）。確認方法: `WorkspaceState::Archived` 分岐が merge-only のフローに留まっていることを確認する。
- [ ] 1.3 ファイル状態ベースで archived と判定されるテストを追加する（`src/execution/state.rs`）。確認方法: 新規テスト名と期待値を確認する。

## 2. Validation
- [ ] 2.1 `cargo test test_detect_workspace_state_archived` を実行し、archive 判定テストが通ることを確認する。
