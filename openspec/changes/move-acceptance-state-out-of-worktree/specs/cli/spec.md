## MODIFIED Requirements

### Requirement: Git Uncommitted Changes Error Message

Git backend で未コミット変更がある場合、CLI は詳細なエラーメッセージを表示しなければならない（SHALL）。
未追跡ファイルの判定では `.gitignore` と `.git/info/exclude` の除外を適用しなければならない（MUST）。
Conflux 自身が resume / archive safety のために保持する acceptance state は、Git worktree 内の untracked/uncommitted artifact として merge blocking の原因になってはならない（MUST NOT）。

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

#### Scenario: Conflux acceptance state does not surface as merge-blocking worktree dirtiness
- **GIVEN** Conflux persists acceptance state for a parallel workspace
- **WHEN** merge readiness or dirty-worktree status is evaluated for that workspace or base branch
- **THEN** Conflux-generated acceptance state does not appear as a blocking worktree change
- **AND** real user-authored uncommitted changes continue to be reported
