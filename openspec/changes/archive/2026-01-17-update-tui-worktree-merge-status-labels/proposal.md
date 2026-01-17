# Change: TUI worktree merge status labels

## Why
Worktreeビューのマージ状態ラベル表記を小文字に統一し、表示の一貫性を高めるため。

## What Changes
- Worktreeビューのマージ進行中ラベルを小文字の "merging" に統一する
- Worktreeビューのマージ済みラベルを小文字の "merged" に統一する
- マージ状態の表示仕様を更新する

## Impact
- Affected specs: tui-worktree-view, tui-worktree-merge
- Affected code: src/tui/render.rs, src/tui/state/mod.rs, src/tui/state/events.rs, src/tui/runner.rs
