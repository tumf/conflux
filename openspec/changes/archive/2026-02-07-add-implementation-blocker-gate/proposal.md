# Change: apply 実装不可エスカレーションの acceptance ゲート追加

## Why
apply の実装中に仕様矛盾や外部制限が顕在化した場合、現在は FAIL → apply ループへ戻る挙動しかなく、無駄な反復や誤った修正ループが発生しやすい。実装不可であることを apply からエスカレーションし、acceptance が妥当性を評価した上でループを停止できるようにする。

## What Changes
- apply が実装不可を判断した場合に構造化されたエスカレーション（Implementation Blocker）を記録する
- acceptance が実装不可エスカレーションを評価し `ACCEPTANCE: BLOCKED` を返せるようにする
- `BLOCKED` 判定時は当該 change の apply ループを停止し、ワークスペースを保持する
- serial/parallel の両モードで同一の停止挙動を提供する

## Impact
- Affected specs: cli, parallel-execution, agent-prompts
- Affected code: src/acceptance.rs, src/orchestration/acceptance.rs, src/serial_run_service.rs, src/orchestrator.rs, src/parallel/mod.rs, .opencode/commands/cflx-apply.md, .opencode/commands/cflx-accept.md
