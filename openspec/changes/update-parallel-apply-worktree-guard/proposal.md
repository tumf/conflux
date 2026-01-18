# Change: parallel apply の worktree 実行ガードを追加

## Why
parallel 実行時に apply が base リポジトリ上で実行されると、作業ツリーが汚れて意図しない変更が混入します。実行ディレクトリの検証不足により発生しうるため、事前検証で fail-fast し原因を明確化します。

## What Changes
- parallel 実行の apply で worktree パス検証を追加し、base 実行を検知したら即時失敗する
- 失敗時に change_id と実行パスを含むエラーを出力する
- 検証成功時は worktree パスをログ出力する

## Impact
- Affected specs: parallel-execution
- Affected code: parallel apply 実行経路（parallel executor / VCS worktree 判定ユーティリティ）
