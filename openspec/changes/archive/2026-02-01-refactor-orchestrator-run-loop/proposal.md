# Change: オーケストレーションrunループの分割

## Why
`Orchestrator::run` が巨大で責務が混在しており、今後の保守や安全な変更が難しくなっています。

## What Changes
- run ループの初期化/停止判定/変更選定/結果処理を小さなヘルパーに分割する
- 既存の挙動・イベント順序・ログ出力を維持する

## Impact
- Affected specs: code-maintenance
- Affected code: src/orchestrator.rs
