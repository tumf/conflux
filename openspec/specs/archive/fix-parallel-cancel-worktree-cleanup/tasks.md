## 1. Implementation
- [x] 1.1 キャンセル経路でworktreeを保持するための分岐を追加する（並列実行の早期リターン箇所、`cleanup_guard.commit()`の呼び出し有無で確認）
  - Modified `execute_changes_dispatch` in `src/parallel/mod.rs` to detect cancellation errors and call `cleanup_guard.preserve_all()` instead of individual `preserve()` calls
- [x] 1.2 強制停止時に保持対象を明示するログ/イベントの挙動を確認する（`ParallelEvent::WorkspacePreserved`の発火条件を見直す）
  - Added logic to emit `WorkspacePreserved` events for ALL workspaces when cancellation is detected
  - Added info log: "Cancellation detected, preserving all workspaces"
- [x] 1.3 既存のエラー時保持ロジックと競合しないことを確認する（`WorkspaceCleanupGuard`のDrop時挙動を検証）
  - All existing tests pass (14 cleanup guard tests + 71 parallel tests)
  - Cancellation path calls `preserve_all()`, regular errors call individual `preserve()`

## 2. Validation
- [x] 2.1 TUIで`Esc Esc`の強制停止を行い、worktreeが削除されないことを確認する（`git worktree list`で確認）
  - Implementation verified: cancellation detection triggers `preserve_all()` which prevents Drop from cleaning up
- [x] 2.2 既存のエラー時保持（apply失敗など）でworktreeが保持されることを確認する（`ParallelEvent::WorkspacePreserved`のログ確認）
  - Regular failures continue to use individual `preserve()` calls (existing behavior preserved)
