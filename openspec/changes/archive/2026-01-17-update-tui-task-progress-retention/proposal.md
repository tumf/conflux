# Change: TUIの最終タスク進捗保持の改善

## Why
TUIで完了済みの変更が `Archived` / `Merged` に遷移した後、進捗が 0/0 と表示されるケースがあり、実際の完了状況が失われてしまうためです。

## What Changes
- TUIが `ApplyCompleted` または `ArchiveStarted` 時に tasks.md を再読込し、最終進捗を確定保存する
- `Archived` / `Merged` で進捗が 0/0 の場合、アーカイブ先 tasks.md から補完する
- tasks.md の探索順序を worktree と base repo を踏まえて明確化する

## Impact
- Affected specs: `cli`
- Affected code: `src/tui/state/events.rs`, `src/tui/state/mod.rs`（予定）
