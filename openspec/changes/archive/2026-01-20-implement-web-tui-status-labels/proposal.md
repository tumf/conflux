# Change: TUI/Web UIのステータス語彙・表示形式を実装に反映する

## Why
既存の仕様ではTUI/Web UIのステータス語彙と表示形式が更新済みだが、実装側には旧語彙（processing/completed）や旧表示形式が残っている。仕様と実装の乖離を解消し、運用時にフェーズが判別できる表示へ統一する。

## What Changes
- TUIのQueueStatus表示語彙を新語彙（not queued, queued, applying, accepting, archiving, resolving, completed, archived, merged, merge wait, error）に合わせて更新する
- TUIの状態遷移イベントをapply/acceptance/archive/resolveのフェーズ表示へ対応させる
- TUIのステータス表示を`status:iteration`形式に更新する
- Web UIのステータス語彙と集計ロジックを新語彙に合わせて更新する
- Web UIのステータス表示を`status:iteration`形式に更新する

## Impact
- Affected specs: cli, web-monitoring
- Affected code: src/tui/types.rs, src/tui/render.rs, src/tui/state/events/*.rs, web/app.js
