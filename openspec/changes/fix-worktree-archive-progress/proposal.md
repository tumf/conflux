# Change: Worktree側アーカイブ進捗の復元

## Why
TUIでArchive中にマージ前のchangeが0/4として表示され、worktree側にしか存在しない最新のtasks.mdが参照されないため、進捗表示の信頼性が落ちる。

## What Changes
- worktreeが存在するchangeについてはtasks.mdをworktree側からのみ読み取り、ベースツリーを参照しない
- Archived/Merged表示の進捗もworktree側のarchive済みtasks.mdから一貫して取得する
- 既存の重複実装やデッドコードがあれば整理する

## Impact
- Affected specs: parallel-execution
- Affected code: task_parser, tui auto-refresh, tui state update
