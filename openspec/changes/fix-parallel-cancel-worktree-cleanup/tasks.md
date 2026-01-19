## 1. Implementation
- [ ] 1.1 キャンセル経路でworktreeを保持するための分岐を追加する（並列実行の早期リターン箇所、`cleanup_guard.commit()`の呼び出し有無で確認）
- [ ] 1.2 強制停止時に保持対象を明示するログ/イベントの挙動を確認する（`ParallelEvent::WorkspacePreserved`の発火条件を見直す）
- [ ] 1.3 既存のエラー時保持ロジックと競合しないことを確認する（`WorkspaceCleanupGuard`のDrop時挙動を検証）

## 2. Validation
- [ ] 2.1 TUIで`Esc Esc`の強制停止を行い、worktreeが削除されないことを確認する（`git worktree list`で確認）
- [ ] 2.2 既存のエラー時保持（apply失敗など）でworktreeが保持されることを確認する（`ParallelEvent::WorkspacePreserved`のログ確認）
