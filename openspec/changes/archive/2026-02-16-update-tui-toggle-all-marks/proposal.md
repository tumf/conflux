# Change: TUI の全マーク/全アンマーク切替を追加

## Why
複数の change をまとめて実行マークする操作がなく、選択作業が手作業で非効率です。1キーで全件を切り替えられるようにして、TUI 操作の負担を下げます。

## What Changes
- Changes ビューに全マーク/全アンマークのトグル操作を追加する
- トグル対象は実行マーク可能な change に限定し、状態に応じて全件を切り替える
- Changes パネルのキーヒントに新しい操作を表示する

## Impact
- Affected specs: `tui-architecture`, `tui-key-hints`
- Affected code: `src/tui/state.rs`, `src/tui/key_handlers.rs`, `src/tui/render.rs`
