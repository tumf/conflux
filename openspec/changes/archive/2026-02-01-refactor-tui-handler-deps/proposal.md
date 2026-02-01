# Change: TUI ハンドラ依存の循環解消

## Why
TUI の runner/command_handlers/key_handlers 間に循環依存があり、モジュール分割や変更の安全性が低下しています。

## What Changes
- runner に依存しているヘルパーを専用モジュールへ移動する
- 既存のキー操作・表示・状態遷移の挙動を維持する

## Impact
- Affected specs: tui-architecture
- Affected code: src/tui/runner.rs, src/tui/command_handlers.rs, src/tui/key_handlers.rs
