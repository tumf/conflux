## RENAMED Requirements
- FROM: `### Requirement: 選択中changeのworktree削除操作を提供する`
- TO: `### Requirement: 選択中worktreeの削除操作を提供する`

## MODIFIED Requirements
### Requirement: 選択中worktreeの削除操作を提供する
TUIは選択中worktreeを削除する操作を提供し、削除前に確認を行わなければならない（SHALL）。

#### Scenario: Dキーで削除確認を出す
- **GIVEN** TUIがWorktreesビューである
- **AND** 選択中worktreeが削除可能である（main ではなく、処理中のchangeに紐づかない）
- **WHEN** WorktreesビューでDキーを押す
- **THEN** 削除確認ダイアログが表示される

#### Scenario: 確認後にworktreeを削除する
- **GIVEN** 削除確認ダイアログで同意する
- **WHEN** 削除処理が実行される
- **THEN** 対象worktreeが削除され、Worktrees一覧から消える

#### Scenario: worktree一覧が空の場合の削除操作
- **GIVEN** TUIがWorktreesビューである
- **AND** worktree一覧が空である
- **WHEN** WorktreesビューでDキーを押す
- **THEN** 何も起こらない
