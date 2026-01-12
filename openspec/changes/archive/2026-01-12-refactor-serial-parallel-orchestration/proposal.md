# Change: serial/parallel 実行フローの重複解消

## Why
serial/parallel モード間で apply・archive・進捗更新の実装が重複しており、変更のたびに両方を更新する必要があるため保守コストと不整合リスクが高い。

## What Changes
- serial/parallel 共通の実行フロー（apply/archiving/状態更新）を共有関数に整理する
- 共有化によって責務の境界を明確化し、モード固有の差分のみを残す
- 既存の挙動・ログ・イベントの互換性は維持する

## Impact
- Affected specs: `specs/code-maintenance/spec.md`, `specs/parallel-execution/spec.md`
- Affected code: `src/orchestrator.rs`, `src/parallel/`, `src/execution/` 周辺
