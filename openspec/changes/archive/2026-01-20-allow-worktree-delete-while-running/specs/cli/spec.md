## MODIFIED Requirements
### Requirement: 実行中changeのworktree削除を禁止する
TUIはProcessing/Running中のchangeに対してworktree削除を許可してはならない（MUST NOT）。
ただし、削除対象worktreeがChanges一覧に存在しない、またはNotQueuedのchangeに紐づく場合は、実行中であっても削除を許可しなければならない（MUST）。

#### Scenario: 実行中の未関連worktreeを削除できる
- **GIVEN** TUIがRunning中である
- **AND** 選択中worktreeがChanges一覧に存在しない、またはNotQueuedのchangeに紐づく
- **WHEN** WorktreesビューでDキーを押して削除を確認する
- **THEN** worktree削除が実行される
- **AND** 削除後にworktree一覧が更新される

#### Scenario: 実行中のqueued/processing系worktreeは削除できない
- **GIVEN** TUIがRunning中である
- **AND** 選択中worktreeがQueued/Processing/Archiving/Resolving/Accepting/MergeWaitのchangeに紐づく
- **WHEN** WorktreesビューでDキーを押す
- **THEN** 削除は行われず、禁止メッセージが表示される
