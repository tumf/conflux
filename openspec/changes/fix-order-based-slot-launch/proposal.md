# Change: Order-based実行で空きスロット数まで同時起動する

## Why
order-based 並列実行が 1 件ずつしか起動されず、max_concurrent_workspaces の上限を活かせないため、期待される並列処理が行われていない。

## What Changes
- order-based の選定ロジックが空きスロット数に応じて複数の change を同時起動するようにする
- 依存関係が解決済みの change のみを空きスロット数まで起動する
- 既存のデバウンスや再分析の仕組みは維持する

## Impact
- Affected specs: parallel-execution
- Affected code: src/parallel/mod.rs
