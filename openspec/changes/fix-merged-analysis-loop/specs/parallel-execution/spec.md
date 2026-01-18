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

### Requirement: Parallel Analysis Targeting
並列実行のanalysisはqueuedのchangeのみを対象にしなければならない（MUST）。

実行中のchangeが存在せず、queuedのchangeも空の場合、システムはオーケストレーションを終了しなければならない（MUST）。

analysis対象をqueuedに限定するため、queuedに含まれないchange（例: merged済みchange、実行済みchange、削除済みchange）はanalysis対象から除外されなければならない（MUST）。

queuedのchangeが空の場合、analysisを実行してはならない（MUST）。

#### Scenario: queuedのみがanalysis対象になる
- **GIVEN** queuedにchangeが存在する
- **AND** queued以外に実行中のchangeが存在する
- **WHEN** 並列実行がanalysisを開始する
- **THEN** analysis対象はqueuedのchangeのみになる

#### Scenario: queued外のchangeはanalysis対象から除外される
- **GIVEN** queuedに含まれないchangeが存在する
- **AND** queuedには別のchangeが存在する
- **WHEN** 並列実行がanalysisを開始する
- **THEN** queued外のchangeはanalysis対象から除外される

#### Scenario: queuedが空ならanalysisを実行しない
- **GIVEN** queuedのchangeが存在しない
- **WHEN** 並列実行がanalysisを開始しようとする
- **THEN** analysisを実行しない

#### Scenario: 実行中とqueuedが空なら終了する
- **GIVEN** 実行中のchangeが存在しない
- **AND** queuedのchangeも空である
- **WHEN** 並列実行ループが次のanalysisを開始しようとする
- **THEN** analysisを実行しない
- **AND** オーケストレーションは完了状態になる
