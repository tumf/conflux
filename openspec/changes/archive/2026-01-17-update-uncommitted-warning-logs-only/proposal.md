# Change: Uncommitted Changes 警告をログのみで扱う

## Why
TUI で未コミット警告がポップアップ表示されると操作を遮り、実行継続の意図と矛盾するため、ログ通知に限定して体験を改善する。

## What Changes
- TUI では未コミット警告をポップアップ表示せず、Logs のみで警告を記録する
- CLI の警告表示は現状維持する

## Impact
- Affected specs: parallel-execution
- Affected code: `src/tui/state/events.rs`, `src/tui/state/mod.rs` (tests)
