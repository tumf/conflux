## MODIFIED Requirements

### Requirement: Parallel apply runs in worktree

parallel mode の apply コマンドは、対象 change の worktree ディレクトリで実行しなければならない（MUST）。これにより base リポジトリの作業ツリーに直接変更が入らないようにする。

#### Scenario: 共通ループを経由しても worktree 内で apply が実行される

- **GIVEN** parallel mode で change が実行対象に選ばれている
- **WHEN** 共通 apply ループが apply コマンドを実行する
- **THEN** 実行ディレクトリは worktree パスである
- **AND** base リポジトリの作業ツリーは変更されない
