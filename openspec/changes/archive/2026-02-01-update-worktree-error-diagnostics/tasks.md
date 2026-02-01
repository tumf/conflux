## 1. VCS エラー文脈の拡充
- [x] 1.1 `VcsError::Command` にコマンド文字列・作業ディレクトリ・stderr/stdout を含める
  - 検証: `src/vcs/commands.rs` にテスト `test_vcs_error_includes_command_context` を追加し、`cargo test vcs_error_includes_command_context` が通る

## 2. worktree add 失敗の分類と再試行
- [x] 2.1 `git worktree add` 失敗の stderr から代表的な原因を分類する（パス重複、ブランチ重複、無効な参照、権限）
  - 検証: `src/vcs/git/commands/worktree.rs` のテスト `test_worktree_add_error_classification` を追加し、`cargo test worktree_add_error_classification` が通る
- [x] 2.2 stale な worktree 参照の場合に `git worktree prune` と安全な再試行を 1 回だけ実行する
  - 検証: `src/vcs/git/commands/worktree.rs` のテスト `test_worktree_add_retry_on_stale_path` を追加し、`cargo test worktree_add_retry_on_stale_path` が通る
- [x] 2.3 再試行が失敗した場合は元のエラーと分類結果を TUI/ログに残す
  - 検証: `src/vcs/git/commands/worktree.rs` のテスト `test_worktree_add_retry_preserves_error` を追加し、`cargo test worktree_add_retry_preserves_error` が通る

## 3. TUI ログ表示の改善
- [x] 3.1 ログバッファはメッセージ全文を保持し、レンダリング時に折り返し表示する
  - 検証: `src/tui/state/logs.rs` にテスト `test_log_wrap_preserves_full_message` を追加し、`cargo test log_wrap_preserves_full_message` が通る
- [x] 3.2 折り返し表示でスクロールの挙動が崩れないことを確認する
  - 検証: `src/tui/render.rs` のレンダリングで `Paragraph::wrap` を使用していることを確認し、`cargo test tui_log_wrap_scroll` が通る
