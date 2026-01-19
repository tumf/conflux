# Change: Archive/Resolve中のtasks進捗が0/0に戻る問題の修正

## Why
archive/resolving中にtasks.mdの読み取りが一時的に失敗すると、進捗が0/0として表示されるケースがあり、実際の進捗が失われたように見えるためです。

## What Changes
- archive/resolving中の進捗更新は、取得失敗(0/0)時に既存の進捗を保持する
- TUI/Webの自動更新・イベント処理を同一のルールに統一する
- worktree/archived fallback で取得した進捗の上書き条件を明確化する

## Impact
- Affected specs: tui-architecture, web-monitoring, parallel-execution
- Affected code: TUI state refresh, WebState refresh, progress update logic
