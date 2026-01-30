# Change: 並列スケジューラの再分析ループ分割

## Why
`execute_with_order_based_reanalysis` が過度に長く、ネストも深いため修正の安全性が低下しています。再分析・ディスパッチ・完了処理を分割し保守性を高めます。

## What Changes
- スロット算出、再分析トリガ、ディスパッチ選定、完了処理をヘルパーに抽出する
- 既存の非ブロッキング性と再分析の起動条件を維持する

## Impact
- Affected specs: `parallel-execution`
- Affected code: `src/parallel/mod.rs`
