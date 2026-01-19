# Change: Preserve worktrees on TUI force stop

## Why
TUIでEsc Escの強制停止を行った際に、実行中のworktreeが`WorkspaceCleanupGuard`のDrop処理で削除され、再開や調査ができなくなる。

## What Changes
- 並列実行のキャンセル経路ではworktreeを削除せず保持する
- 強制停止後もworktreeを再開に利用できることを保証する

## Impact
- Affected specs: parallel-execution, workspace-cleanup
- Affected code: src/parallel/mod.rs, src/parallel/cleanup.rs
