# Change: Resolve完了後のTUI表示状態退行を修正する

## Why

Resolve（マージ解決）が正常に完了し、worktreeのクリーンアップも成功しているにもかかわらず、TUI上でchangeが「UNCOMMITTED QUEUED」と表示され続けるバグがある。根本原因はレースコンディション：`ResolveCompleted`イベントでMergedに遷移した直後に、同一resolve呼出し内の後続検証（`is_archive_commit_complete`）が失敗して`ResolveFailed`イベントが発火し、状態がMergedからMergeWaitに退行する。さらにauto-refreshの`auto_clear_merge_wait`がworktree不在を検出してQueuedに遷移させ、`is_parallel_eligible=false`によりUNCOMMITTEDバッジが付加される。

## What Changes

- `handle_resolve_failed`: 既にMergedに遷移したchangeへのResolveFailed適用をガードする
- `apply_merge_wait_status`: Merged状態をガード条件に追加し、auto-refreshによるMergeWait退行を防止する
- `auto_clear_merge_wait`: MergeWait→Queued遷移時にMerged状態のchangeが対象にならないよう明示的にガードする（防御的措置）

## Impact

- Affected specs: tui-worktree-merge
- Affected code: `src/tui/state.rs` (`handle_resolve_failed`, `apply_merge_wait_status`, `auto_clear_merge_wait`)
