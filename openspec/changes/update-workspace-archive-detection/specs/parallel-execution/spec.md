## MODIFIED Requirements
### Requirement: Workspace State Detection
既存workspaceの再開時に、archive 状態を以下の3段階で判定しなければならない（MUST）。

- archiving: `openspec/changes/archive/<date>-<change_id>` または `openspec/changes/archive/<change_id>` が worktree に存在するが、`Archive: <change_id>` のコミットが未完了、または worktree が clean でない
- archived: worktree が clean であり、worktree HEAD のツリーに `openspec/changes/<change_id>` が存在せず、`openspec/changes/archive/<date>-<change_id>` または `openspec/changes/archive/<change_id>` が存在する。`Archive: <change_id>` のコミットが HEAD 以外でも archived と判定しなければならない（MUST）。
- merged: base ブランチに `Archive: <change_id>` が存在し、`openspec/changes/<change_id>` が存在しない

archiving の場合は apply を再実行せず、archive ループに進めなければならない（MUST）。

#### Scenario: worktreeにアーカイブ済みファイルがありコミットが未完了
- **GIVEN** worktree 内に `openspec/changes/archive/<date>-<change_id>` が存在する
- **AND** `Archive: <change_id>` のコミットが未完了である
- **WHEN** `detect_workspace_state(change_id, workspace_path, base_branch)` が呼ばれる
- **THEN** 状態は archiving と判定される
- **AND** apply ではなく archive ループに進む

#### Scenario: ArchiveコミットがHEAD以外でもarchivedと判定する
- **GIVEN** worktree HEAD のツリーに `openspec/changes/archive/2024-01-15-test-change` が存在する
- **AND** `openspec/changes/test-change` が存在しない
- **AND** `Archive: test-change` のコミットが履歴に存在するが HEAD のコミット件名は別である
- **WHEN** `detect_workspace_state(test-change, workspace_path, base_branch)` が呼ばれる
- **THEN** 状態は archived と判定される
