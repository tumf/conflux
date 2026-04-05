## MODIFIED Requirements

### Requirement: Git Uncommitted Changes Error Message

Git backend で未コミット変更がある場合、CLI は詳細なエラーメッセージを表示しなければならない（SHALL）。
未追跡ファイルの判定では `.gitignore` と `.git/info/exclude` の除外を適用しなければならない（MUST）。
Conflux が workspace local な内部運用ファイルを生成する場合、それらの dirty-worktree 除外は repository-tracked `.gitignore` ではなく、対象 workspace の実効 `info/exclude` で管理されなければならない（MUST）。

#### Scenario: Error message format
- **WHEN** parallel execution is attempted with Git backend
- **AND** uncommitted changes exist
- **THEN** the error message includes:
  - Problem description
  - Resolution method (commit or stash)
  - Specific command examples

#### Scenario: Untracked files also trigger error
- **WHEN** parallel execution is attempted with Git backend
- **AND** only untracked files exist
- **THEN** the same error message is displayed
- **AND** files in `.gitignore` と `.git/info/exclude` は除外される

#### Scenario: Workspace local internal artifact is excluded without tracked ignore
- **GIVEN** Conflux generates `.cflx/acceptance-state.json` inside a workspace
- **AND** the repository `.gitignore` does not contain an ignore rule for that path
- **WHEN** Git dirty-worktree status is evaluated for that workspace
- **THEN** `.cflx/acceptance-state.json` is excluded via the workspace's effective `info/exclude`
- **AND** it does not appear in the untracked file list
