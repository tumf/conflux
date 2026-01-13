## ADDED Requirements

### Requirement: TUIのChange一覧にworktree存在を表示する
TUIのChange一覧は、各changeに紐づくworktreeの有無を識別できるインジケータを表示しなければならない（SHALL）。

#### Scenario: worktreeが存在するchangeの表示
- **GIVEN** 対象changeにworktreeが存在する
- **WHEN** TUIのChange一覧を表示する
- **THEN** そのchangeの行にworktreeインジケータが表示される

#### Scenario: worktreeが存在しないchangeの表示
- **GIVEN** 対象changeにworktreeが存在しない
- **WHEN** TUIのChange一覧を表示する
- **THEN** そのchangeの行にworktreeインジケータは表示されない

### Requirement: 選択中changeのworktree削除操作を提供する
TUIは選択中changeに紐づくworktreeを削除する操作を提供し、削除前に確認を行わなければならない（SHALL）。

#### Scenario: Dキーで削除確認を出す
- **GIVEN** 選択中changeにworktreeが存在する
- **WHEN** SelectモードでDキーを押す
- **THEN** 削除確認ダイアログが表示される

#### Scenario: 確認後にworktreeを削除する
- **GIVEN** 削除確認ダイアログで同意する
- **WHEN** 削除処理が実行される
- **THEN** 対象worktreeが削除され、Change一覧からインジケータが消える

#### Scenario: worktreeが存在しない場合の削除操作
- **GIVEN** 選択中changeにworktreeが存在しない
- **WHEN** SelectモードでDキーを押す
- **THEN** 削除は行われず、存在しない旨の通知が表示される

### Requirement: 実行中changeのworktree削除を禁止する
TUIはProcessing/Running中のchangeに対してworktree削除を許可してはならない（MUST NOT）。

#### Scenario: Processing中に削除を試みる
- **GIVEN** 選択中changeがProcessing/Running中である
- **WHEN** SelectモードでDキーを押す
- **THEN** 削除は行われず、禁止メッセージが表示される
