# Change: Resolve中のMキー操作を無効化する

## Why
並列モードでのresolve実行中にMキーが同時に動作すると、base branch側のマージ処理が競合する可能性があります。UI上で操作を抑止し、衝突リスクとユーザー混乱を防ぎます。

## What Changes
- resolve実行中はChanges/Worktrees両ビューでMキー操作を無効化する
- Mキーのヒント表示もresolve中は出さない
- 無効化理由をユーザーに警告メッセージとして提示する

## Impact
- Affected specs: tui-key-hints, tui-worktree-merge
- Affected code: src/tui/state/mod.rs, src/tui/state/events.rs, src/tui/runner.rs, src/tui/render.rs
