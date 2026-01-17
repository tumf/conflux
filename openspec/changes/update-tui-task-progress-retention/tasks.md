## 1. TUI進捗保持ロジックの整理
- [x] 1.1 ApplyCompleted / ArchiveStarted で最終進捗を再読込し保存する
- [x] 1.2 Archived / Merged 表示で 0/0 の場合にアーカイブ先 tasks.md から補完する
- [x] 1.3 update_changes で Archived の進捗を 0/0 に上書きしない

## 2. テストと検証
- [x] 2.1 TUI state の進捗保持テストを追加する
- [x] 2.2 npx @fission-ai/openspec@latest validate update-tui-task-progress-retention --strict を実行する
