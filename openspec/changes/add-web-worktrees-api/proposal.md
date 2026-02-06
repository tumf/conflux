# Change: Web UI/APIでTUI Worktrees Viewの機能を提供

## Why
TUIのWorktrees Viewで可能な情報取得や操作をWeb UI/APIから行えず、Web監視時に同等の運用ができないためです。

## What Changes
- Web監視APIにworktree一覧/作成/削除/マージ/コマンド実行のエンドポイントを追加する
- Web UIにWorktrees Viewと操作UIを追加し、TUIと同じ制約でガードする
- Web state更新にworktree再取得と操作結果を反映し、TUIと同一語彙で通知する

## Impact
- Affected specs: web-monitoring
- Affected code: src/web/**, src/tui/**, src/vcs/**, web/**
