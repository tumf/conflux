## 1. Implementation
- [x] 1.1 キャンセル経路でworktreeを保持するための分岐を追加する（並列実行の早期リターン箇所、`cleanup_guard.commit()`の呼び出し有無で確認）
- [x] 1.2 強制停止時に保持対象を明示するログ/イベントの挙動を確認する（`ParallelEvent::WorkspacePreserved`の発火条件を見直す）
- [x] 1.3 既存のエラー時保持ロジックと競合しないことを確認する（`WorkspaceCleanupGuard`のDrop時挙動を検証）
- [x] 1.4 Run cargo fmt to format code
- [x] 1.5 Run cargo clippy to check for lints

## 2. Validation
- [x] 2.1 Run cargo test to verify all tests pass
- [x] 2.2 既存のエラー時保持（apply失敗など）でworktreeが保持されることを確認する（既存テストで確認）
