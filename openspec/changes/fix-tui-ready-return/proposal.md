# Change: TUI完了時にReadyへ復帰

## Why
処理が完了してもヘッダーが Ready に戻らず、Stopped 扱いのままになるケースがあり、正常完了後の操作が分かりにくくなるためです。

## What Changes
- 正常完了時は `AllCompleted` で Ready/Select へ戻すイベントフローを明確化する
- オーケストレータ終了後に `Stopped` を送る挙動を廃止する（停止要求時のみ `Stopped`）

## Impact
- Affected specs: `specs/cli/spec.md`
- Affected code: `src/tui/command_handlers.rs`, `src/tui/state/events/processing.rs`
