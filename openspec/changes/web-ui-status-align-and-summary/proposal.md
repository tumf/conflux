# Change: Web UI status alignment and summary layout

## Why
Web UI の状態表示が TUI と一致しておらず、ユーザーが同じオーケストレーションの進行状況を正しく把握しづらい。ステータス名の差異や一覧の情報量過多により視認性が下がっているため、Web UI を TUI の表示・用語に合わせて整理する必要がある。

## What Changes
- Web UI のステータス表示を TUI の QueueStatus 表示に合わせる（pending/in_progress/complete を廃止し、not queued などに統一）
- Web UI の全体進捗を最上位に配置し、changes 一覧は情報を絞って一覧性を向上させる
- archiving/resolving/merge wait/merged など TUI と同じ状態を表示する
- 各ループのイテレーション番号を change 行に表示する
- 実行マーク SPC / Approve @ の操作 UI は通常は畳み、必要時のみ展開する
- アイコンを使って状態とアクションの一覧性を強化する

## Impact
- Affected specs: web-monitoring
- Affected code: web/index.html, web/app.js, web/style.css, src/web/state.rs
