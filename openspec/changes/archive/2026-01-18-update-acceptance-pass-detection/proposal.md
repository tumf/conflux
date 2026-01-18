# Change: acceptance 判定の PASS 検知を装飾付きでも正しく行う

## Why
acceptance の出力が装飾付き (例: **ACCEPTANCE: PASS**) の場合、現行の厳密一致判定に失敗し、誤って tasks.md に失敗追記が行われます。これにより適切な PASS 判定ができず、運用上の混乱が発生します。

## What Changes
- acceptance 出力の判定を装飾付きでも検知できるようにする
- 解析対象を stdout に限定せず、判定に必要な出力を考慮する
- PASS 判定時に tasks.md を更新しない動作を保証する

## Impact
- Affected specs: cli, parallel-execution
- Affected code: src/acceptance.rs, src/orchestration/acceptance.rs
