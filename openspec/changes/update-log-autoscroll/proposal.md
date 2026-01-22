# Change: ログのオートスクロール停止時に表示を固定する

## Why
ログのオートスクロールを無効にしても新しいログ追加で表示が流れ続けるため、任意のログ位置を確認しづらい。

## What Changes
- オートスクロール無効時は新規ログ追加やバッファトリムが発生しても表示中のログ範囲を固定する
- 画面外に押し出された場合は残存ログの最古行にクランプし、オートスクロールは再有効化しない

## Impact
- Affected specs: `openspec/specs/tui-architecture/spec.md`
- Affected code: `src/tui/state/logs.rs`, `src/tui/state/mod.rs`
