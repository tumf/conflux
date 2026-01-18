# Change: acceptance integration

## Why
acceptance の実行がオーケストレーションの実行経路に統合されておらず、apply→acceptance→archive のループが成立していないため、実装と検証が分断されている。

## What Changes
- 逐次実行の apply 成功後に acceptance を必ず実行し、結果でループ分岐する
- 並列実行の apply 成功後に acceptance を必ず実行し、結果でループ分岐する
- acceptance の成功/失敗/実行失敗に応じて履歴・ログ・状態遷移を明確化する

## Impact
- Affected specs: cli, parallel-execution
- Affected code: orchestrator run loop, parallel executor, acceptance orchestration
