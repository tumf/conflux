# Change: Parallel mode uses commit snapshot eligibility

## Why
並列モードで未コミットの change が worktree に存在せず、実行が失敗するケースがあるためです。実行対象をコミット済みの change に限定し、UI でも明確に区別する必要があります。

## What Changes
- 並列モードの change 対象を `HEAD` のコミットツリー起点で判定する
- 未コミットの change は並列モードで選択不可にし、行をグレーアウトする
- 未コミットの change に `UNCOMMITED` バッジを表示する
- 並列実行側でも未コミット change を除外し、警告ログを出す

## Impact
- Affected specs: parallel-execution, tui-key-hints
- Affected code: `src/vcs/git/commands.rs`, `src/parallel_run_service.rs`, `src/tui/render.rs`, `src/tui/state/*.rs`, `src/tui/runner.rs`
