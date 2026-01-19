# Change: TUIアーカイブ途中の進捗ゼロ化を防止

## Why
TUIでアーカイブ途中にtasks.mdが移動されると一時的に進捗が0/0へ表示され、完了状況の可視性が失われるため。

## What Changes
- アーカイブ途中（worktree上でファイル移動済み・コミット未完了）でも進捗を保持する処理を追加する。
- 進捗取得が0/0になる場合は既存の進捗を維持する挙動を明示する。

## Impact
- Affected specs: `openspec/specs/tui-architecture/spec.md`
- Affected code: `src/tui/state/events.rs`, `src/tui/state/mod.rs`
