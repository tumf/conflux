# Change: Align parallel hook execution with event reporting

## Why
parallel apply/archive 共通ループに移行する中で、hook 実行と ParallelEvent の通知が分散し、実装理解と保守が難しくなっています。実行フローを統一し、既存の挙動を変えずに hook とイベントの整合性を明確にする必要があります。

## What Changes
- parallel apply/archive の共通ループ内で hook 実行と ParallelEvent 発行を統一する
- hook の実行タイミングとイベント通知の対応関係を明文化する
- 既存の hook 成功/失敗時の挙動は変更しない

## Impact
- Affected specs: parallel-execution
- Affected code: parallel apply/archive 実行ループ、hook 実行処理、ParallelEvent 発行
