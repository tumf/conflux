# Change: parallel apply を worktree で実行する

## Why
parallel apply が base リポジトリで実行されるため、worktree ではなく base の作業ツリーが更新される。これは並列実行の隔離前提と合致せず、意図しない変更が残るリスクがある。

## What Changes
- parallel apply の実行ディレクトリを worktree に統一する
- apply 実行時に worktree 以外へ書き込まないことを明確化する

## Impact
- Affected specs: parallel-execution
- Affected code: src/parallel/executor.rs
