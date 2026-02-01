# Change: Orchestrator の run ループ分割

## Why
`src/orchestrator.rs` の `run` が大規模で、状態更新や結果分岐が混在しています。責務ごとに分割して変更の安全性を高めます。

## What Changes
- キャンセル／イテレーション制御の判定をヘルパー関数へ抽出する
- `ChangeProcessResult` の分岐処理をヘルパー関数に分離する
- 重複する状態更新処理を共通化する

## Impact
- Affected specs: `code-maintenance`
- Affected code: `src/orchestrator.rs`, `src/orchestration/state/*`（必要に応じて）
