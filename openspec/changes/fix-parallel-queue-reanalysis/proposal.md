# Change: Parallel queue reanalysis during execution

## Why
実行中に追加した提案が queued のまま滞留し、仕様で定義された再分析（キュー変更後のデバウンスとスロット駆動）が起きないため、並列実行の期待動作と一致しません。

## What Changes
- 実行中のキュー追加/削除を監視し、キュー変更から一定時間後に再分析が走るようにする
- 空きスロットが発生した時点で `order` に従い次の変更を即時起動する
- 既存のバッチ処理境界に依存した再分析待ちを解消する

## Impact
- Affected specs: parallel-execution
- Affected code: src/parallel/mod.rs, src/tui/orchestrator.rs, src/parallel_run_service.rs
