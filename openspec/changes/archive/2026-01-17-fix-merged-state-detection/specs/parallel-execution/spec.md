## MODIFIED Requirements
### Requirement: Workspace State Detection

既存workspaceの再開時に、`WorkspaceState::Merged` と判定するのは、`Archive: <change_id>` がbaseブランチに存在し、かつ `openspec/changes/<change_id>` が存在しない場合に限らなければならない（MUST）。

#### Scenario: Archiveコミットがあるがchangesが残っている場合はMergedと判定しない
- **GIVEN** baseブランチに `Archive: <change_id>` のコミットが存在する
- **AND** `openspec/changes/<change_id>` ディレクトリが存在する
- **WHEN** `detect_workspace_state(change_id, workspace_path, base_branch)` が呼ばれる
- **THEN** 状態は `WorkspaceState::Merged` と判定されない
- **AND** `WorkspaceState::Archived` または `WorkspaceState::Applied` を判定するための次の検査へ進む
