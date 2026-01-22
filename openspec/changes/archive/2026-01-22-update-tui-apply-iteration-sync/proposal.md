# Change: TUI apply イテレーション表示の同期修正

## Why
Logs では apply の反復が進んでいるのに、Changes 一覧が古いイテレーションを表示し続けるため、進捗表示の信頼性が下がっています。

## What Changes
- apply 出力イベントから最新の iteration 番号を TUI の Changes 一覧へ反映する
- Changes 一覧のステータス表示が最新の iteration に追従することを保証する

## Impact
- Affected specs: cli
- Affected code: src/execution/apply.rs, src/parallel/output_bridge.rs, src/tui/state/events/output.rs
