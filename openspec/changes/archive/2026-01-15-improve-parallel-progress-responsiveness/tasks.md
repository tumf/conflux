## 1. Worktree Path Resolver の実装

- [x] 1.1 `src/vcs/git/mod.rs` に `get_worktree_path_for_change()` 関数を追加
- [x] 1.2 `git worktree list --porcelain` をパースして change_id から worktree path を取得
- [x] 1.3 Unit test: worktree path 解決のテストを追加

## 2. Worktree 優先 Task Parser の実装

- [x] 2.1 `src/task_parser.rs` に `parse_change_with_worktree_fallback()` 関数を追加
- [x] 2.2 worktree path があればそこから読み、なければ base tree から読むロジック実装
- [x] 2.3 Unit test: worktree 優先と fallback のテストを追加

## 3. TUI Auto-Refresh の更新

- [x] 3.1 `src/tui/runner.rs` の auto-refresh ループ（line 181 付近）を更新
- [x] 3.2 `openspec::list_changes_native()` の後で各 change の worktree 進捗を enrichment
- [x] 3.3 Error handling: worktree 読み取り失敗時は既存進捗を保持

## 4. 統合テストと検証

- [x] 4.1 Integration test: TUI で worktree 進捗が即座に反映されることを確認
- [x] 4.2 手動テスト: 並列モードで apply 実行中に TUI で進捗を確認
- [x] 4.3 `cargo test` で全テスト通過を確認
- [x] 4.4 `cargo fmt --check` と `cargo clippy` でコードスタイルを確認
