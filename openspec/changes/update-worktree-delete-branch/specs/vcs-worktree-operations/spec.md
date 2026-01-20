## MODIFIED Requirements

### Requirement: Worktree setup script execution

システムは worktree 作成時にリポジトリ直下の `.wt/setup` スクリプトを検出し、存在する場合は実行しなければならない（MUST）。

セットアップ実行時、システムは環境変数 `ROOT_WORKTREE_PATH` にベースリポジトリ（ソースツリー）のパスを設定しなければならない（MUST）。

`.wt/setup` が存在しない場合、システムはセットアップ処理を実行してはならない（MUST NOT）。

#### Scenario: setupスクリプトが存在する場合に実行される
- **GIVEN** リポジトリ直下に `.wt/setup` が存在する
- **WHEN** 新しい worktree が作成される（TUIの「+」を含む）
- **THEN** `.wt/setup` が実行される
- **AND** `ROOT_WORKTREE_PATH` がベースリポジトリのパスとして設定される

#### Scenario: setupスクリプトが存在しない場合は何もしない
- **GIVEN** リポジトリ直下に `.wt/setup` が存在しない
- **WHEN** 新しい worktree が作成される（TUIの「+」を含む）
- **THEN** セットアップ処理は実行されない

#### Scenario: setupスクリプトが失敗した場合はエラーになる
- **GIVEN** `.wt/setup` が存在する
- **AND** スクリプトが非ゼロ終了コードで終了する
- **WHEN** 新しい worktree が作成される（TUIの「+」を含む）
- **THEN** worktree作成は失敗として扱われる
- **AND** 失敗理由がログに記録される

### Requirement: Worktree delete removes branch

Worktreesビューの削除操作でworktreeを削除するとき、システムは対応するローカルブランチも削除しなければならない（MUST）。

ブランチが存在しない、または削除に失敗した場合でも、worktree削除は成功として扱い、ブランチ削除失敗は警告ログとして記録しなければならない（MUST）。

#### Scenario: worktree削除時にブランチも削除される
- **GIVEN** Worktreesビューでworktree削除を実行する
- **AND** 対象worktreeにローカルブランチが紐づいている
- **WHEN** worktree削除処理が完了する
- **THEN** ローカルブランチも削除される
- **AND** worktree削除とブランチ削除の成功ログが記録される

#### Scenario: ブランチ削除に失敗してもworktree削除は成功扱い
- **GIVEN** Worktreesビューでworktree削除を実行する
- **AND** 対象ブランチが既に削除済みである
- **WHEN** worktree削除処理が完了する
- **THEN** worktree削除は成功として扱われる
- **AND** ブランチ削除失敗の警告ログが記録される
