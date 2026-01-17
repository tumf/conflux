## 1. 実装
<<<<<<< HEAD
- [ ] 1.1 parallel 用の CommandQueue 共有インスタンスを用意し、apply/archive から利用できるようにする
- [ ] 1.2 `execute_apply_in_workspace` を CommandQueue 経由の実行に切り替える（stagger + retry + streaming）
- [ ] 1.3 `execute_archive_in_workspace` を CommandQueue 経由の実行に切り替える（stagger + retry + streaming）
- [ ] 1.4 リトライ時のログ出力が TUI/CLI で確認できるようにイベント/ログ配線を整理する

## 2. 既存挙動の維持
- [ ] 2.1 worktree 内で実行されること（parallel apply runs in worktree）を維持する
- [ ] 2.2 既存の hook 実行順序・イベント通知が変わらないことを確認する

## 3. 検証
- [ ] 3.1 既存の並列実行系テストを確認し、必要なら追加テストを設計する
- [ ] 3.2 `cargo test` を実行して差分が問題ないことを確認する
=======
- [x] 1.1 parallel 用の CommandQueue 共有インスタンスを用意し、apply/archive から利用できるようにする
- [x] 1.2 `execute_apply_in_workspace` を CommandQueue 経由の実行に切り替える（stagger + retry + streaming）
- [x] 1.3 `execute_archive_in_workspace` を CommandQueue 経由の実行に切り替える（stagger + retry + streaming）
- [x] 1.4 リトライ時のログ出力が TUI/CLI で確認できるようにイベント/ログ配線を整理する

## 2. 既存挙動の維持
- [x] 2.1 worktree 内で実行されること（parallel apply runs in worktree）を維持する
- [x] 2.2 既存の hook 実行順序・イベント通知が変わらないことを確認する

## 3. 検証
- [x] 3.1 既存の並列実行系テストを確認し、必要なら追加テストを設計する
- [x] 3.2 `cargo test` を実行して差分が問題ないことを確認する
>>>>>>> fix-parallel-command-queue
