## MODIFIED Requirements
### Requirement: Workspace State Detection
既存workspaceの再開時に、archive 状態を以下の3段階で判定しなければならない（MUST）。

- archiving: `openspec/changes/archive/<date>-<change_id>` が worktree に存在するが、`Archive: <change_id>` のコミットが未完了
- archived: `Archive: <change_id>` のコミットが完了し、`openspec/changes/<change_id>` が存在しない
- merged: base ブランチに `Archive: <change_id>` が存在し、`openspec/changes/<change_id>` が存在しない

archiving の場合は apply を再実行せず、archive ループに進めなければならない（MUST）。

並列実行のanalysis前に、mergedと判定できるchange（baseブランチに`Archive: <change_id>`が存在し、`openspec/changes/<change_id>`が存在しないもの）は対象から除外しなければならない（MUST）。

除外の結果、対象changeが空になった場合、システムはオーケストレーションを終了しなければならない（MUST）。

#### Scenario: worktreeにアーカイブ済みファイルがありコミットが未完了
- **GIVEN** worktree 内に `openspec/changes/archive/<date>-<change_id>` が存在する
- **AND** `Archive: <change_id>` のコミットが未完了である
- **WHEN** `detect_workspace_state(change_id, workspace_path, base_branch)` が呼ばれる
- **THEN** 状態は archiving と判定される
- **AND** apply ではなく archive ループに進む

#### Scenario: merged済みchangeがanalysis対象から除外される
- **GIVEN** base ブランチに `Archive: <change_id>` が存在する
- **AND** `openspec/changes/<change_id>` が存在しない
- **WHEN** 並列実行がanalysisを開始する
- **THEN** `change_id` はanalysis対象から除外される
- **AND** 除外理由がログまたはイベントで確認できる

#### Scenario: 全件mergedの場合は並列実行が終了する
- **GIVEN** 対象のchangeがすべてmerged判定される
- **WHEN** 並列実行のanalysisループが開始される
- **THEN** 以降のanalysisを実行しない
- **AND** オーケストレーションは完了状態になる
