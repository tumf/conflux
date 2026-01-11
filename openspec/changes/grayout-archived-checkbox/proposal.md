# Change: archived 状態の checkbox をグレーアウト

## Why

TUI で change が archived 状態になった後も `[x]` が表示され続けるため、処理中の change と区別しにくい。archived 済みの change は操作不可能なので、checkbox を視覚的に区別する必要がある。

## What Changes

- TUI のレンダリングにおいて、`queue_status == Archived` の change は checkbox をグレー色（`Color::DarkGray`）で表示
- 選択モード・実行モード両方で適用

## Impact

- Affected specs: `cli`
- Affected code: `src/tui/render.rs`
