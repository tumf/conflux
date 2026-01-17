## Context
TUI の進捗表示は tasks.md を読み取るが、`Archived` / `Merged` 遷移後に tasks.md が移動して 0/0 表示になることがある。

## Goals / Non-Goals
- Goals:
  - `Archived` / `Merged` の進捗が 0/0 に戻らず、最後の進捗を維持する
  - tasks.md の探索順序を明確化する
- Non-Goals:
  - CLI/Web の進捗表示変更
  - tasks.md のフォーマット変更

## Decisions
- ApplyCompleted / ArchiveStarted 時点で tasks.md を再読込し、ChangeState に最終進捗を保存する
- Archived / Merged で 0/0 の場合はアーカイブ先 tasks.md を参照して補完する
- 探索順序は worktree → worktree repo → base repo を優先する

## Risks / Trade-offs
- archive の瞬間に tasks.md が存在しない場合は最後の進捗を保持する前提になる

## Migration Plan
- 既存の進捗保持ロジックを壊さないように段階的に追加する

## Open Questions
- なし
