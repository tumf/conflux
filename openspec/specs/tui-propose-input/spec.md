# tui-propose-input Specification

## Purpose
TBD - created by archiving change add-propose-command. Update Purpose after archive.
## Requirements
### Requirement: `+` による提案作成フローの起動

TUIはSelectモードで `+` キーを押したとき、以下の条件をすべて満たす場合に限り、提案作成フローを開始しなければならない（SHALL）。

- 現在の作業ディレクトリが Git リポジトリ上である
- 設定で `worktree_command` が定義されている

提案作成フローでは、一時ディレクトリ配下に Git worktree を作成し、その worktree を **子プロセスの `cwd`** として `worktree_command` を実行しなければならない（SHALL）。

この worktree は **detached HEAD** ではなく、必ず新しいブランチに紐づく worktree でなければならない（MUST）。
ブランチ名は `oso-session-<rand>` 形式でなければならない（MUST）。

#### Scenario: Gitリポジトリ上で `worktree_command` が設定されている

- **GIVEN** TUIがSelectモードである
- **AND** 現在の作業ディレクトリがGitリポジトリ上である
- **AND** 設定で `worktree_command` が定義されている
- **WHEN** ユーザーが `+` キーを押す
- **THEN** 一時ディレクトリ配下に新しいGit worktreeが作成される
- **AND** 作成されるworktreeはブランチに紐づいている（detached ではない）
- **AND** ブランチ名は `oso-session-<rand>` 形式である
- **AND** `worktree_command` が作成したworktreeを `cwd` として実行される
- **AND** 作成したworktreeは削除されずに残る

#### Scenario: Gitリポジトリ上でない場合は無操作

- **GIVEN** TUIがSelectモードである
- **AND** 現在の作業ディレクトリがGitリポジトリ上でない
- **WHEN** ユーザーが `+` キーを押す
- **THEN** 何も起こらない

#### Scenario: `worktree_command` 未設定の場合は無操作

- **GIVEN** TUIがSelectモードである
- **AND** 現在の作業ディレクトリがGitリポジトリ上である
- **AND** `worktree_command` が設定されていない
- **WHEN** ユーザーが `+` キーを押す
- **THEN** 何も起こらない

### Requirement: Runningモードでは提案作成不可

TUIはRunningモードで `+` キーを押した場合、何も起こしてはならない（SHALL NOT）。

#### Scenario: Runningモードでは無操作

- **GIVEN** TUIがRunningモードである
- **WHEN** ユーザーが `+` キーを押す
- **THEN** 何も起こらない
