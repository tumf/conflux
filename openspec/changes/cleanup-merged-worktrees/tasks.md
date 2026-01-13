# タスク一覧: マージ完了後のGit worktreeクリーンアップ

## 準備
- [ ] 現行の並列実行フローにおけるworktreeライフサイクル（作成→apply→archive→merge→cleanup）を確認
- [ ] `src/parallel/cleanup.rs` と `src/vcs/git/mod.rs` の責務（どこが最終的にworktreeを消すべきか）を整理

## 実装
- [ ] マージ成功（MergeCompleted相当）のタイミングで、対象changeのworktree削除を実行する
- [ ] worktreeパスを保持できない経路がある場合、`git worktree list --porcelain` から該当worktreeを特定して削除する
- [ ] worktree削除後に関連ブランチも削除する
- [ ] 削除失敗時は warn ログを出し、処理を継続する

## テスト/検証
- [ ] 並列実行E2E（Git worktree）で、マージ成功後に `git worktree list` に対象worktreeが残らないことを確認
- [ ] 既存の再開（resume）シナリオで、不要なworktreeが残っていても異常終了しないことを確認
- [ ] `cargo test` を実行

## 完了条件
- [ ] マージ成功した変更のworktreeが自動で削除される
- [ ] `git worktree list` に不要なworktreeが残らない
- [ ] テストが通る
