## MODIFIED Requirements

### Requirement: Parallel apply runs in worktree
parallel mode の apply コマンドは、対象 change の worktree ディレクトリで実行しなければならない（MUST）。これにより base リポジトリの作業ツリーに直接変更が入らないようにする。

#### Scenario: apply 実行が worktree 内で行われる
- **GIVEN** parallel mode で change が実行対象に選ばれている
- **WHEN** apply コマンドが実行される
- **THEN** 実行ディレクトリは worktree パスである
- **AND** base リポジトリの作業ツリーは変更されない

#### Scenario: デフォルトworkspaceディレクトリの解決
- **GIVEN** `workspace_base_dir` が未設定
- **WHEN** parallel 実行が workspace の作成先を決定する
- **THEN** デフォルトディレクトリは設定仕様に従って解決される
- **AND** worktree は `<data_dir>/conflux/worktrees/<project_slug>` 配下に作成される
