## MODIFIED Requirements
### Requirement: Workspace State Detection
既存workspaceの再開時に、archive 状態をコミットメッセージではなく **コミットされたファイルの状態** で判定しなければならない（MUST）。

判定基準（すべて worktree HEAD ツリーのファイル状態で判定）:

- archiving: worktree が dirty（未コミットの変更がある）かつ `openspec/changes/<change_id>` が存在せず、archive エントリ（`openspec/changes/archive/<date>-<change_id>` または `openspec/changes/archive/<change_id>`）が存在する
- archived: worktree が clean であり、`openspec/changes/<change_id>` が存在せず、archive エントリが存在する
- merged: base ブランチの HEAD ツリーに archive エントリが存在し、`openspec/changes/<change_id>` が存在しない

archiving の場合は apply を再実行せず、archive ループに進めなければならない（MUST）。
archived の場合は apply/archive を再実行せず、merge のみ実行しなければならない（MUST）。

#### Scenario: worktreeがdirtyでarchiveエントリがあればarchiving
- **GIVEN** worktree 内の `openspec/changes/<change_id>` が存在しない
- **AND** worktree 内に `openspec/changes/archive/<date>-<change_id>` が存在する
- **AND** worktree が dirty である（未コミットの変更がある）
- **WHEN** `detect_workspace_state(change_id, workspace_path, base_branch)` が呼ばれる
- **THEN** 状態は archiving と判定される
- **AND** apply ではなく archive ループに進む

#### Scenario: worktreeがcleanでarchiveエントリがあればarchived
- **GIVEN** worktree が clean である
- **AND** worktree HEAD ツリーに `openspec/changes/test-change` が存在しない
- **AND** worktree HEAD ツリーに `openspec/changes/archive/2024-01-15-test-change` が存在する
- **WHEN** `detect_workspace_state(test-change, workspace_path, base_branch)` が呼ばれる
- **THEN** 状態は archived と判定される
- **AND** apply/archive を再実行せず merge のみ実行する

#### Scenario: baseブランチにarchiveエントリがあればmerged
- **GIVEN** base ブランチの HEAD ツリーに `openspec/changes/archive/2024-01-15-test-change` が存在する
- **AND** base ブランチの HEAD ツリーに `openspec/changes/test-change` が存在しない
- **WHEN** `detect_workspace_state(test-change, workspace_path, base_branch)` が呼ばれる
- **THEN** 状態は merged と判定される
