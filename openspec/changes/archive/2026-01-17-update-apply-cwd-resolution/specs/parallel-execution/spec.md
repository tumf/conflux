## MODIFIED Requirements

### Requirement: Parallel apply runs in worktree

parallel mode の apply コマンドは、対象 change の worktree ディレクトリで実行しなければならない（MUST）。これにより base リポジトリの作業ツリーに直接変更が入らないようにする。

#### Scenario: 共通ループでも worktree が強制される

- **GIVEN** parallel mode で change が実行対象に選ばれている
- **WHEN** apply/archive の共通ループが実行される
- **THEN** 実行ディレクトリは worktree パスである
- **AND** repo root での実行は許可されない
