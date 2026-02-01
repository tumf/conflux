# Change: ParallelRunService の準備処理共通化

## Why
ParallelRunService の開始準備が複数経路で重複しており、修正漏れのリスクが高い状態です。

## What Changes
- コミットツリーに存在しない change の除外と警告通知を共通化する
- 並列実行の開始準備をヘルパーに集約し、挙動を維持する

## Impact
- Affected specs: parallel-execution
- Affected code: src/parallel_run_service.rs
