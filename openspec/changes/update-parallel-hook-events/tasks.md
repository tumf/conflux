## 1. Implementation
- [x] 1.1 parallel apply/archive 共通ループで hook 実行と ParallelEvent 発行を統合する
- [x] 1.2 hook 実行タイミングごとのイベント発行を共通ループ経由に統一する
- [x] 1.3 既存の hook 成功/失敗時の挙動が維持されることを確認する

## 2. Tests
- [x] 2.1 parallel apply/archive の hook 実行時に HookStarted/HookCompleted/HookFailed が発行されることを確認する
- [x] 2.2 continue_on_failure の挙動が変わらないことを確認する
