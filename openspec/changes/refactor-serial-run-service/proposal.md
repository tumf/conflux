# Change: SerialRunService パターンで serial 実行を統合する

## Why
run と TUI の serial 実行ロジックが分岐しており、同一挙動の保証や保守が難しい。ParallelRunService と同様の共通サービス層を導入し、serial の共通化を進める。

## What Changes
- SerialRunService を新設し、serial 実行の共通フローを集約する
- run と TUI の serial 実行は共通サービスを利用し、出力・UI・キュー制御などの差分のみを薄いアダプタ層に分離する
- 既存の挙動（フック、acceptance、archive 優先、iteration 制御、履歴記録）を維持する

## Impact
- Affected specs: specs/code-maintenance/spec.md, specs/cli/spec.md, specs/tui-architecture/spec.md
- Affected code: src/orchestrator.rs, src/tui/orchestrator.rs, src/orchestration/, 新規 src/serial_run_service.rs
