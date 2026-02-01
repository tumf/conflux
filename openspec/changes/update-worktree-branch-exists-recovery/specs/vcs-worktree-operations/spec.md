## ADDED Requirements
### Requirement: worktree add のブランチ既存エラー分類

システムは `git worktree add` が「a branch named ... already exists」相当の stderr を返した場合、原因を「ブランチ既存」として分類しなければならない（MUST）。

#### Scenario: ブランチ既存エラーは分類される
- **GIVEN** `git worktree add` が「a branch named 'x' already exists」相当の stderr を返す
- **WHEN** worktree 作成に失敗する
- **THEN** 原因は「ブランチ既存」として分類される

### Requirement: ブランチ既存時の安全な worktree 再作成

`git worktree add <path> -b <branch> <base>` がブランチ既存で失敗した場合、システムは当該ブランチが他の worktree にチェックアウトされていないことを確認できたときに限り、`git worktree add <path> <branch>` を 1 回だけ再試行しなければならない（MUST）。

他の worktree にチェックアウト済みであることが確認できた場合、システムは再試行を行ってはならない（MUST NOT）。

#### Scenario: ブランチ既存かつ未チェックアウトなら再試行で成功する
- **GIVEN** `refs/heads/<branch>` は存在するが、どの worktree にもチェックアウトされていない
- **AND** `git worktree add <path> -b <branch> <base>` がブランチ既存で失敗する
- **WHEN** worktree 作成が再試行される
- **THEN** `git worktree add <path> <branch>` が 1 回だけ実行される

#### Scenario: ブランチ既存かつ他 worktree でチェックアウト済みなら再試行しない
- **GIVEN** `refs/heads/<branch>` が他の worktree でチェックアウトされている
- **AND** `git worktree add <path> -b <branch> <base>` がブランチ既存で失敗する
- **WHEN** worktree 作成が失敗する
- **THEN** 再試行は行われない
