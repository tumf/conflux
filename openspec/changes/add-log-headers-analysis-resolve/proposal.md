# Change: Add analysis/resolve log headers

## Why
分析ループと resolve のログが視認しづらく、反復回数や対象変更が即座に判別できないためです。ログヘッダを一貫した形式で付与し、運用時の追跡性を高めます。

## What Changes
- 解析ログに `[analysis:N]` 形式のヘッダを表示する
- resolve ログに `[{change_id}:resolve:N]` 形式のヘッダを表示する
- 解析/resolve のヘッダ表示ルールを TUI ログパネルの仕様に明記する

## Impact
- Affected specs: cli
- Affected code: src/events.rs, src/tui/state/events.rs, src/tui/render.rs, src/parallel/conflict.rs
