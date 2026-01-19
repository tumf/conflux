## 1. Implementation
- [x] 1.1 `src/vcs/git/commands.rs` の `check_merge_conflicts` が `git merge-tree --write-tree --merge-base <base> <branch1> <branch2>` の形式で実行されることを確認し、実装を修正する（関数内のコマンド引数とエラー処理を確認）。
- [x] 1.2 `check_merge_conflicts` の競合検出が stdout を優先して判定し、exit code 1 を「競合あり」として扱うことを確認する（stdout/stderr のどちらを解析するかと分岐を確認）。
- [x] 1.3 競合判定失敗時の debug ログに stdout/stderr/exit code が含まれることを確認する（ログ出力の内容を確認）。
- [x] 1.4 関連するテストを追加または更新し、`cargo test` もしくは該当テストが通ることを確認する（実行コマンドと結果を記録）。
