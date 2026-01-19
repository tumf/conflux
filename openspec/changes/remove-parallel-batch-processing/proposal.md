# Change: 並列実行のバッチ/グループ前提を明確に廃止する

## Why
並列実行がバッチ/グループ完了を前提にしているため、実行中のキュー追加や再分析がバッチ完了まで待たされ、仕様の「実行中でも監視し、空きスロットで即時に起動する」要件を満たせていない。バッチ/グループという概念自体を明示的に廃止し、スロット駆動の連続ディスパッチへ統一する必要がある。

## What Changes
- バッチ/グループ完了を前提にした実行フローを削除し、空きスロット駆動の連続ディスパッチに統一する
- 依存関係の失敗チェックはグループ単位ではなく、ディスパッチ時の個別評価に切り替える
- バッチ/グループ用語に依存したログ・イベント・完了判定をスロット駆動の用語へ置き換える

## Impact
- Affected specs: parallel-execution
- Affected code: src/parallel/mod.rs, src/parallel_run_service.rs, src/tui/orchestrator.rs, src/parallel/executor.rs
