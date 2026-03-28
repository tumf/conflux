## ADDED Requirements

### Requirement: Dashboard Worktree Management UI

Server Mode Dashboard はプロジェクトごとのWorktree管理UIを提供しなければならない（MUST）。

#### Scenario: Worktree一覧が表示される
- **GIVEN** ユーザーがダッシュボードでプロジェクトを選択している
- **WHEN** Worktreesタブに切り替える
- **THEN** 該当プロジェクトのWorktree一覧が表示される
- **AND** 各Worktreeにブランチ名、状態バッジ、コンフリクト情報が表示される

#### Scenario: Worktreeを作成できる
- **GIVEN** ユーザーがWorktreesタブを表示している
- **WHEN** 作成ボタンをクリックしてchange_idを入力する
- **THEN** 新しいWorktreeが作成される
- **AND** 一覧にリアルタイムで反映される

#### Scenario: Worktreeを削除できる
- **GIVEN** メインでないWorktreeが一覧に存在する
- **WHEN** ユーザーが削除ボタンをクリックして確認する
- **THEN** Worktreeが削除される
- **AND** 一覧から即座に消える

#### Scenario: Worktreeブランチをマージできる
- **GIVEN** コンフリクトがなく先行コミットがあるWorktreeが存在する
- **WHEN** ユーザーがマージボタンをクリックする
- **THEN** ブランチがベースブランチにマージされる
- **AND** 成功通知が表示される

### Requirement: Dashboard Changes/Worktrees Tab Navigation

Server Mode Dashboard は Changes と Worktrees をタブ切り替えで表示しなければならない（MUST）。

#### Scenario: デスクトップでタブ切り替えできる
- **GIVEN** ユーザーがデスクトップレイアウトでダッシュボードを表示している
- **WHEN** Changes/Worktreesタブをクリックする
- **THEN** 対応するパネルが表示される

#### Scenario: モバイルでWorktreesタブが利用できる
- **GIVEN** ユーザーがモバイルレイアウトでダッシュボードを表示している
- **WHEN** 下部のWorktreesタブをタップする
- **THEN** Worktreesパネルが表示される
