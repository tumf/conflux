# タスク一覧: マージ完了後のGit worktreeクリーンアップ

## 準備
- [x] 現行の並列実行フローにおけるworktreeライフサイクル（作成→apply→archive→merge→cleanup）を確認
- [x] `src/parallel/cleanup.rs` と `src/vcs/git/mod.rs` の責務（どこが最終的にworktreeを消すべきか）を整理

## 実装
- [x] マージ成功（MergeCompleted相当）のタイミングで、対象changeのworktree削除を実行する
- [x] worktreeパスを保持できない経路がある場合、`git worktree list --porcelain` から該当worktreeを特定して削除する
- [x] worktree削除後に関連ブランチも削除する
- [x] 削除失敗時は warn ログを出し、処理を継続する

## テスト/検証
- [x] 並列実行E2E（Git worktree）で、マージ成功後に `git worktree list` に対象worktreeが残らないことを確認
- [x] 既存の再開（resume）シナリオで、不要なworktreeが残っていても異常終了しないことを確認
- [x] `cargo test` を実行

## 完了条件
- [x] マージ成功した変更のworktreeが自動で削除される
- [x] `git worktree list` に不要なworktreeが残らない
- [x] テストが通る
