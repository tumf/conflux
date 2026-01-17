# Change: Enforce parallel concurrency limit across workspace creation

## Why
parallel modeで最大同時実行数が守られず、デフォルト設定でも多数のworktreeが同時に作成・実行されてしまうため、リソース制御が破綻しています。上限が効いているというユーザー期待と挙動が一致しないため、並列実行の信頼性を改善します。

## What Changes
- worktree作成・apply・archiveを含む並列実行の同時数上限を厳密に適用する
- デフォルト値（3）や設定値/CLI指定値がTUIとCLIの双方で一貫して反映されるようにする
- 並列実行時の上限超過を防止するための仕様を明文化する

## Impact
- Affected specs: parallel-execution, configuration, cli
- Affected code: src/parallel/mod.rs, src/tui/orchestrator.rs, src/orchestrator.rs
