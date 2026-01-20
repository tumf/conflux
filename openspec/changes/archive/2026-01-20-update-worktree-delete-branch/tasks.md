## 1. Implementation
- [x] 1.1 Worktreesビューの削除コマンドでブランチ名を特定できるようにする（`git worktree list --porcelain` の結果から削除対象パスに対応するブランチを抽出し、該当しない場合はdetached扱いでスキップする）; 検証: 対象ロジックが `src/tui/runner.rs` の削除処理から呼ばれていることを確認する
- [x] 1.2 worktree削除後にブランチ削除を実行し、失敗時はwarnログを残して処理を継続する; 検証: `src/vcs/git/mod.rs` の既存ブランチ削除関数を呼び出していることを確認する
- [x] 1.3 削除完了ログに「worktree削除成功」と「ブランチ削除成功」を分けて出力する; 検証: `src/tui/runner.rs` のログ出力が意図通り増えていることを確認する

## 2. Validation
- [x] 2.1 `cargo fmt` を実行しフォーマット差分がないことを確認する
- [x] 2.2 `cargo clippy -- -D warnings` を実行し警告がないことを確認する
- [x] 2.3 `cargo test` を実行し既存テストが通ることを確認する
