# Change: Add acceptance loop iteration header

## Why
acceptance のログから現在のループ番号や再実行時の連続性が読み取れず、apply/acceptance の進行状況を追いづらいためです。ヘッダ表示と番号継続を明示して運用時の可観測性を高めます。

## What Changes
- acceptance ログに `[{change_id}:acceptance:<iteration>]` 形式のヘッダを表示する
- acceptance 失敗で apply に戻った場合でも、acceptance のループ番号をリセットせず引き継ぐ
- acceptance の apply への再投入後も同じ iteration を継続して利用する
- acceptance のログヘッダ規約を仕様として明記する

## Impact
- Affected specs: cli
- Affected code: src/events.rs, src/orchestrator.rs, src/tui/orchestrator.rs, src/parallel/mod.rs, src/tui/render.rs
