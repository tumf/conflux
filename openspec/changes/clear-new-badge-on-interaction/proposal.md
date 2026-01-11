# Change: ユーザー操作時にNEWバッジを消去する

## Why

現在、`tui list` で表示される NEW バッジは Select モードで選択を切り替えたときのみ消える。承認（`@`キー）やキュー追加（Running/Stopped モード）では消えないため、ユーザーが change に対して操作したにも関わらず NEW マークが残り続けてしまう。

## What Changes

- 承認（approve）操作時に NEW バッジを消去する
- キュー追加操作時に NEW バッジを消去する（Running/Stopped モード）
- 既存の Select モードでの選択時の消去動作は維持

## Impact

- Affected specs: `cli` (NEW badge の消去条件を追加)
- Affected code: `src/tui/state/mod.rs` (`toggle_selection`, `update_approval_status`)
