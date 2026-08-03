## MODIFIED Requirements

### Requirement: 選択中worktreeの削除操作を提供する

TUIは通常削除確認と既知dirty内容の破棄確認を区別しなければならない（MUST）。通常確認の`Y`または`S`はdirty-discard permissionを付与してはならない（MUST NOT）。fresh service observationが既知dirtyを返した場合のみ第二確認を表示し、大文字`X`だけを破棄入力として受理しなければならない（MUST）。`S`はskip-teardown選択だけを表し、第二確認まで保持されるが、それ自体はdirty削除を許可してはならない（MUST NOT）。

#### Scenario: Dキーで削除確認を出す
- **GIVEN** TUIがWorktreesビューである
- **AND** 選択中worktreeが削除可能である
- **WHEN** WorktreesビューでDキーを押す
- **THEN** 通常削除確認が表示される
- **AND** `Y`はteardownあり、`S`はskip-teardownであることが表示される

#### Scenario: Yでclean worktreeを削除する
- **GIVEN** 通常削除確認の対象がcleanで削除適格である
- **WHEN** ユーザーが`Y`を押す
- **THEN** teardown後に対象worktreeが削除される

#### Scenario: Yからdirty破棄確認へ進む
- **GIVEN** fresh service observationが対象を既知dirtyと判定する
- **WHEN** 通常確認で`Y`を押す
- **THEN** worktreeはまだ削除されない
- **AND** skip-teardown=falseを保持したdirty破棄確認が表示される
- **WHEN** ユーザーが大文字`X`を押す
- **THEN** teardownと最終再検証後にworktreeが削除される

#### Scenario: Sからdirty破棄確認へ進む
- **GIVEN** fresh service observationが対象を既知dirtyと判定する
- **WHEN** 通常確認で`S`を押す
- **THEN** worktreeはまだ削除されない
- **AND** skip-teardown=trueを保持し、teardownも省略されることを示すdirty破棄確認が表示される
- **WHEN** ユーザーが大文字`X`を押す
- **THEN** teardownを実行せず、最終再検証後にworktreeが削除される

#### Scenario: dirty破棄確認はX以外で削除しない
- **GIVEN** dirty破棄確認が表示されている
- **WHEN** ユーザーが`Y`、`S`、小文字`x`、または無関係なキーを押す
- **THEN** 削除は実行されない
- **WHEN** ユーザーが`N`またはEscを押す
- **THEN** 確認は閉じ、worktreeは保持される

#### Scenario: unknown observationは破棄確認へ進まない
- **GIVEN** dirty、commits-ahead、base merge、Git identity、またはbranch refの安全観測を確定できない
- **WHEN** 通常削除またはdirty破棄が要求される
- **THEN** dirty破棄確認へ進まず削除を拒否する
- **AND** 理由を表示する

#### Scenario: dispatch前のactive遷移を拒否する
- **GIVEN** dirty破棄確認が表示されている
- **WHEN** 対象changeがdispatch前にactiveまたはdeletingへ遷移する
- **THEN** 削除を拒否しworktreeを保持する

#### Scenario: worktree一覧が空の場合の削除操作
- **GIVEN** TUIがWorktreesビューである
- **AND** worktree一覧が空である
- **WHEN** WorktreesビューでDキーを押す
- **THEN** 何も起こらない
