# Change: TUI のキーイベント／コマンド処理の分割

## Why
`run_tui_loop` のキーイベント処理と TuiCommand 処理が肥大化しており、変更時の影響範囲の把握が難しくなっています。分割して保守性と可読性を高めます。

## What Changes
- キーイベント処理をヘルパー関数に分割し、`run_tui_loop` の責務を整理する
- TuiCommand の処理を専用のヘルパー関数に抽出する
- 既存のショートカット／表示／状態遷移の挙動は変更しない

## Impact
- Affected specs: `tui-architecture`
- Affected code: `src/tui/runner.rs`
