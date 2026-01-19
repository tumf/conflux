## MODIFIED Requirements
### Requirement: Archive Commit Completion via resolve_command
archive ループに入る前に tasks.md の完了率が100%であることを検証し、未完了または欠落している場合は archive に進んではならない（MUST）。

検証時は worktree 内の `openspec/changes/{change_id}/tasks.md` を優先して読み取り、存在しない場合は worktree 内の `openspec/changes/archive/<date>-<change_id>/tasks.md` をフォールバックとして読み取らなければならない（MUST）。

#### Scenario: tasks.md が未完了の場合は archive を停止する
- **GIVEN** tasks.md の完了率が100%ではない
- **WHEN** archive が開始される
- **THEN** archive コマンドは実行されない
- **AND** エラーとして記録される

#### Scenario: archive 側の tasks.md をフォールバックして検証する
- **GIVEN** worktree 内の `openspec/changes/{change_id}/tasks.md` が存在しない
- **AND** worktree 内の `openspec/changes/archive/<date>-<change_id>/tasks.md` が存在する
- **WHEN** archive 前のタスク検証が実行される
- **THEN** archive 側の tasks.md を読み取り、完了率を判定する
