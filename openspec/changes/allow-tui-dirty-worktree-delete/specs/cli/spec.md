## MODIFIED Requirements

### Requirement: 選択中worktreeの削除操作を提供する

TUIは選択中worktreeを削除する操作を提供し、削除前に確認を行わなければならない（SHALL）。既知の未コミットまたは未追跡変更を含むworktreeについては、通常確認だけで削除してはならず（MUST NOT）、恒久的な破棄を明示する専用の第二確認と通常削除とは異なる確認入力を要求しなければならない（MUST）。第二確認後も、削除直前にpathへ紐づくbranch identityと削除適格性を再検証しなければならない（MUST）。

#### Scenario: Dキーで削除確認を出す
- **GIVEN** TUIがWorktreesビューである
- **AND** 選択中worktreeが削除可能である（main ではなく、処理中のchangeに紐づかない）
- **WHEN** WorktreesビューでDキーを押す
- **THEN** 削除確認ダイアログが表示される

#### Scenario: clean worktreeを通常確認後に削除する
- **GIVEN** 削除確認の対象がcleanで削除適格である
- **WHEN** ユーザーが通常削除を確認する
- **THEN** 対象worktreeが削除され、Worktrees一覧から消える

#### Scenario: dirty worktreeは破棄確認へ進む
- **GIVEN** 通常削除確認の対象に既知の未コミットまたは未追跡変更がある
- **WHEN** ユーザーが通常削除を確認する
- **THEN** worktreeは削除されない
- **AND** 変更が恒久的に失われることを示す専用の第二確認が表示される
- **AND** 通常削除とは異なる明示的な破棄入力が案内される

#### Scenario: dirty worktreeの破棄を明示確認する
- **GIVEN** dirty worktreeの専用破棄確認が表示されている
- **AND** 対象のbranch identityと削除適格性が変化していない
- **WHEN** ユーザーが専用の破棄入力を行う
- **THEN** `.wt/teardown`が通常どおり実行される
- **AND** 対象worktreeが削除される
- **AND** dirty内容を意図的に破棄したwarningが記録される
- **AND** Worktrees一覧が更新される

#### Scenario: dirty worktreeの破棄をキャンセルする
- **GIVEN** dirty worktreeの専用破棄確認が表示されている
- **WHEN** ユーザーがNまたはEscを押す
- **THEN** 確認は閉じる
- **AND** worktreeとその内容は保持される

#### Scenario: 第二確認後の状態変化は削除を拒否する
- **GIVEN** dirty worktreeの専用破棄確認が表示されている
- **WHEN** 削除実行前の再観測でbranch identity不一致、main、処理中、削除中、dirty状態不明、base merge中、またはbaseよりaheadであることが判明する
- **THEN** 削除は拒否される
- **AND** worktreeは保持される
- **AND** 操作者に理由が表示される

#### Scenario: worktree一覧が空の場合の削除操作
- **GIVEN** TUIがWorktreesビューである
- **AND** worktree一覧が空である
- **WHEN** WorktreesビューでDキーを押す
- **THEN** 何も起こらない
