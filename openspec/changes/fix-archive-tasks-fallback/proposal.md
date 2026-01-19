# Change: archive側のtasks.mdフォールバック不備を修正

## Why
archive 実行前の tasks.md 完了検証で、archive 済みディレクトリの tasks.md を参照できず、再開時に正しい進捗が判定できないケースがあるため。

## What Changes
- archive 前の tasks.md 検証で、worktree 内の `openspec/changes/archive/<date>-<change_id>/tasks.md` をフォールバックとして参照できるようにする
- フォールバックの参照優先順位を明確化し、worktree 内の最新アーカイブ済み tasks.md を利用する

## Impact
- Affected specs: `specs/parallel-execution/spec.md`
- Affected code: `src/execution/archive.rs`, `src/parallel/executor.rs` 付近
