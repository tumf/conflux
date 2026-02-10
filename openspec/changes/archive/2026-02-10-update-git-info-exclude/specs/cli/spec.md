## MODIFIED Requirements
### Requirement: Git Uncommitted Changes Error Message

Git backend で未コミット変更がある場合、CLI は詳細なエラーメッセージを表示しなければならない（SHALL）。
あわせて未追跡ファイルの判定前に `.git/info/exclude` に `openspec/changes/*/approved` が存在しない場合は追加しなければならない（MUST）。
未追跡ファイルの判定では `.gitignore` と `.git/info/exclude` の除外を適用しなければならない（MUST）。

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

#### Scenario: Missing local exclude entry is appended

- **GIVEN** `.git/info/exclude` に `openspec/changes/*/approved` が存在しない
- **WHEN** 未追跡ファイルの判定が行われる
- **THEN** `.git/info/exclude` に `openspec/changes/*/approved` が 1 行だけ追加される
