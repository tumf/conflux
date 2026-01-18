## MODIFIED Requirements

### Requirement: Workspace State Detection
既存workspaceの再開時に、archive 状態を以下の3段階で判定しなければならない（MUST）。

- archiving: `openspec/changes/archive/<date>-<change_id>` が worktree に存在するが、`Archive: <change_id>` のコミットが未完了
- archived: `Archive: <change_id>` のコミットが完了し、`openspec/changes/<change_id>` が存在しない
- merged: base ブランチに `Archive: <change_id>` が存在し、`openspec/changes/<change_id>` が存在しない

archiving の場合は apply を再実行せず、archive ループに進めなければならない（MUST）。

#### Scenario: worktreeにアーカイブ済みファイルがありコミットが未完了
- **GIVEN** worktree 内に `openspec/changes/archive/<date>-<change_id>` が存在する
- **AND** `Archive: <change_id>` のコミットが未完了である
- **WHEN** `detect_workspace_state(change_id, workspace_path, base_branch)` が呼ばれる
- **THEN** 状態は archiving と判定される
- **AND** apply ではなく archive ループに進む

### Requirement: Archive Commit Completion via resolve_command
archive ループに入る前に tasks.md の完了率が100%であることを検証し、未完了または欠落している場合は archive に進んではならない（MUST）。

#### Scenario: tasks.md が未完了の場合は archive を停止する
- **GIVEN** tasks.md の完了率が100%ではない
- **WHEN** archive が開始される
- **THEN** archive コマンドは実行されない
- **AND** エラーとして記録される
