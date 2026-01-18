# Change: TUI acceptance ステータスの追加

## Why
TUI の進捗表示で acceptance 実行中が他の処理と区別できず、acceptance ループ中であることが分かりにくい。acceptance 実行中に `accepting` を表示することで、現在のフェーズを明確にし運用判断をしやすくする。

## What Changes
- TUI の change ステータスに acceptance 実行中を示す `accepting` を追加する
- acceptance 開始〜完了までの間、該当 change のステータスを `accepting` として表示する
- acceptance 終了後は既存のステータス遷移に戻す

## Impact
- Affected specs: tui-architecture
- Affected code: src/tui/types.rs, src/tui/state/events.rs, src/events.rs, src/orchestration/acceptance.rs, src/parallel/executor.rs
