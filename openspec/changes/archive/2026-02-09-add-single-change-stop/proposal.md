# Change: 実行中changeの単体停止

## Why
実行中のchangeを止めたい場合に全体停止しかできず、他のqueuedがあっても処理が止まってしまいます。実行マークを外してnot queuedへ戻す単体停止を提供することで、誤実行の回避と運用柔軟性を高めます。

## What Changes
- Runningモードで実行中changeを1件だけ停止できる操作を追加する（Spaceキー）
- 停止完了時に当該changeを`not queued`へ戻し、実行マークを解除する
- 停止失敗時はエラーログを残し、状態を保持する
- キーヒントに`Space: stop`を表示する条件を追加する

## Impact
- Affected specs: `specs/tui-architecture/spec.md`, `specs/cli/spec.md`, `specs/tui-key-hints/spec.md`, `specs/parallel-execution/spec.md`
- Affected code: `src/tui/state.rs`, `src/tui/events.rs`, `src/tui/key_handlers.rs`, `src/tui/command_handlers.rs`, `src/tui/render.rs`, `src/tui/orchestrator.rs`, `src/serial_run_service.rs`, `src/parallel/mod.rs`, `src/parallel/executor.rs`
