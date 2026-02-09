# Change: TUIログパネルの表示切替

## Why
ログパネルが常時表示されると、Changes一覧の表示領域が狭くなり、状態確認や選択操作がしづらくなります。作業状況に応じてログを隠せるようにし、一覧とログの両方を使いやすくします。

## What Changes
- Changesビューでログパネルの表示/非表示を切り替えられるようにする（推奨キー: `l`）
- ログパネルの既定状態は表示（有効）とし、ログは非表示中も蓄積される
- キーヒントに `l: logs` を追加する

## Impact
- Affected specs: `specs/cli/spec.md`, `specs/tui-key-hints/spec.md`
- Affected code: `src/tui/state.rs`, `src/tui/key_handlers.rs`, `src/tui/render.rs`
