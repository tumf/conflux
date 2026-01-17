## MODIFIED Requirements

### Requirement: Parallel apply runs in worktree

parallel mode の apply コマンドは、対象 change の worktree ディレクトリで実行しなければならない（MUST）。これにより base リポジトリの作業ツリーに直接変更が入らないようにする。

#### Scenario: apply 実行が共通ループから worktree 内で行われる
- **GIVEN** parallel mode で change が実行対象に選ばれている
- **WHEN** 共通 apply ループが apply コマンドを実行する
- **THEN** 実行ディレクトリは worktree パスである
- **AND** base リポジトリの作業ツリーは変更されない

### Requirement: Git 以外では WIP/スタール検知を無効化

WIP スナップショットとスタール検知は Git バックエンド時のみ有効とし、Git 以外のバックエンドではスキップしなければならない（MUST）。

#### Scenario: Git 以外では WIP スナップショットを作らない
- **GIVEN** Git 以外のバックエンドで apply ループが実行されている
- **WHEN** イテレーションが終了する
- **THEN** WIP スナップショットは作成されない
- **AND** スタール検知は実行されない
