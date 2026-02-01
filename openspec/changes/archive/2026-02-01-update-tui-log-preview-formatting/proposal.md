# Change: TUI Changes一覧のログプレビュー表記を改善

## Why
ログプレビューの相対時間が本文と区別しにくく、またカーソル行では背景色と同化して見えづらい。

## What Changes
- ログプレビューの相対時間を括弧で囲む
- カーソル行のログプレビュー文字色を明るくして可読性を確保する

## Impact
- Affected specs: cli
- Affected code: src/tui/render.rs
