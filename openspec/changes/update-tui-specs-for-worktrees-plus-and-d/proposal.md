# Change: TUIの+とDキー仕様をWorktreesビュー基準に更新

## Why
現在の仕様では `+` と `D` の操作が Select モード前提になっているが、実装は Worktrees ビューを前提としている。
また、`+` による worktree 作成のブランチ名プレフィックスが仕様（`oso-session`）と実装（`ws-session`）で不一致になっている。

## What Changes
- `tui-propose-input` 仕様を Worktrees ビュー基準に更新し、`+` の起動条件と `ws-session-<rand>` ブランチ名を明記する
- `tui-propose-input` の「Runningモードでは提案作成不可」を「Changesビューでは提案作成不可」へ置き換える
- `cli` 仕様の worktree 削除操作（`D`）を Worktrees ビュー前提に更新する

## Impact
- Affected specs: tui-propose-input, cli
- Affected code: なし（仕様の整合のみ）
