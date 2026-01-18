# Change: TUIのProcessing中も進捗を更新する

## Why
TUIではProcessing中の変更の進捗が更新されず、Web UIと表示が一致しないため、実行中の進捗確認が困難です。

## What Changes
- Processing中の変更でもtasks.mdの進捗をTUIに反映する
- 進捗取得に失敗した場合は既存表示を保持する

## Impact
- Affected specs: tui-architecture
- Affected code: src/tui/state/events.rs, src/tui/runner.rs
