# Change: TUI進捗のworktree fallback強化

## Why
archiving/resolving/mergedのタイミングで tasks.md の参照先が移動し、TUIの進捗表示が0/0にリセットされるデグレが発生しているため、正しい進捗保持を復旧する。

## What Changes
- TUIの進捗取得で worktree を優先し、ベースツリーへフォールバックする
- archiving/resolving/merged/archived で tasks.md が一時的に読めない場合も進捗を保持する
- 進捗取得ルールをイベント処理と自動リフレッシュで統一する

## Impact
- Affected specs: tui-architecture
- Affected code: src/tui/state/events.rs, src/tui/state/mod.rs, src/tui/runner.rs, src/task_parser.rs
