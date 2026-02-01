# Change: TUI の error モード遷移と MergeWait 操作性の整合

## Why
change 単位の失敗で TUI 全体が Error モードへ遷移すると、処理継続と操作ヒントの整合が崩れ、`merge wait` の `M` 操作が実行できない状態が発生するため。

## What Changes
- change の `ProcessingError` では AppMode を Error に遷移させず、失敗は change 単位で保持する
- Error モードは致命的なエラーイベントに限定する
- `M: resolve` の表示条件を「実際に resolve 操作が可能な状態」に一致させる

## Impact
- Affected specs: `tui-key-hints`, `tui-error-handling` (new)
- Affected code: `src/tui/state/events/processing.rs`, `src/tui/render.rs`, `src/tui/state/mod.rs`, `src/tui/state/modes.rs`
