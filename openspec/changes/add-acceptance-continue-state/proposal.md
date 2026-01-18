# Change: Add acceptance CONTINUE outcome

## Why
現在の acceptance 判定は PASS/FAIL のみで、調査が終わっていない場合の扱いが明確ではありません。PASS/FAIL 以外の状態を表現できないため、手動再実行やループ条件が曖昧になっています。

## What Changes
- acceptance 判定に CONTINUE を追加し、調査未完了の状態を明示する
- CONTINUE の扱い（再実行・上限・記録）を run ループ仕様に追加する
- acceptance 出力フォーマットとパーサを更新する

## Impact
- Affected specs: `openspec/specs/cli/spec.md`, `openspec/specs/configuration/spec.md`
- Affected code: `src/acceptance.rs`, `src/orchestration/acceptance.rs`, `src/orchestrator.rs`, `src/parallel/`
