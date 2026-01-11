# Change: README.ja.md を README.md と同期

## Why

README.ja.md（日本語版ドキュメント）が README.md（英語版）と比較して古くなっており、最新機能の記載が欠落している。特に Git worktrees による並列実行、新しいフックシステム、CLI オプションの記載に差異がある。

## What Changes

- README.ja.md の「並列実行」セクションに Git worktrees サポートを追加
- README.ja.md の run サブコマンドオプションに `--parallel`, `--max-concurrent`, `--vcs`, `--dry-run` を追加
- README.ja.md の「フック設定」セクションを英語版と一致させる
- 古い/削除されたフック（`on_first_apply`, `on_iteration_start` 等）の記載を削除
- 新しいフック（`on_change_start`, `on_change_end` 等）の記載を追加

## Impact

- Affected specs: documentation
- Affected code: README.ja.md のみ（コード変更なし）
