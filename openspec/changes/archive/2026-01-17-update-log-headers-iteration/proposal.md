# Change: Update analysis/resolve log headers with iteration

## Why
分析および解決のログヘッダにイテレーション番号が表示されず、再実行や繰り返しの判別がしづらい。

## What Changes
- TUIログのヘッダ表示に `[analysis:N]` と `[resolve:N]` を追加する。
- 既存の change_id を含むヘッダ形式は維持する。

## Impact
- Affected specs: tui-architecture
- Affected code: `src/tui/render.rs`
