# Change: Webダッシュボードの最新状態反映を修正

## Why
Web UI が TUI の最新状態を反映せず、手動リロードでも古い情報が表示されます。現在の実装では WebState が実行イベントでのみ更新されるため、TUI の自動更新結果が Web 側に伝わりません。

## What Changes
- TUI 自動更新で取得した変更一覧を WebState に反映する
- `/api/state` と Web UI の手動リロードが最新状態を返すことを保証する

## Impact
- Affected specs: specs/web-monitoring/spec.md
- Affected code: src/tui/runner.rs (自動更新タスク), src/web/state.rs (状態更新の入口)
