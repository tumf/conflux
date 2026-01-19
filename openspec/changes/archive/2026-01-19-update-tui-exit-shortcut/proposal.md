# Change: TUIの終了キーをCtrl+Cのみに変更

## Why
TUIの終了操作をCtrl+Cに統一し、誤操作による終了を減らすため。

## What Changes
- TUIの終了操作をCtrl+Cのみに変更し、qキーでの終了を廃止する
- 終了キー表示（ステータス/フッター/完了メッセージ）をCtrl+C表記に更新する

## Impact
- Affected specs: cli, tui-key-hints
- Affected code: src/tui/runner.rs, src/tui/render.rs
