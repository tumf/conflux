# Tasks

## Phase 1: デバッグログの追加（クラッシュ原因特定）

- [ ] `src/tui/state/mod.rs`の`request_merge_worktree_branch()`にデバッグログ追加（view_mode, worktrees.len, cursor_index, 各条件チェック結果）
- [ ] `src/tui/runner.rs`のMキーハンドリング部分にデバッグログ追加（コマンド送信前後）
- [ ] `src/tui/runner.rs`の`TuiCommand::MergeWorktreeBranch`処理部分にデバッグログ追加
- [ ] 手動テスト: `RUST_LOG=debug`でMキー押下時のログを確認し、処理の流れを追跡

## Phase 2: マージ実行先の修正（クリティカルバグ修正）

- [ ] `src/tui/runner.rs`の`TuiCommand::MergeWorktreeBranch`ハンドラで、`merge_branch(&worktree_path, ...)`を`merge_branch(&merge_repo_root, ...)`に変更
- [ ] 手動テスト: worktree側にuncommitted changesがある状態でMキーを押してマージが成功することを確認
- [ ] 手動テスト: base側にuncommitted changesがある状態でMキーを押して「Working directory is not clean」エラーが表示されることを確認

## Phase 3: エラーメッセージの追加

- [ ] `src/tui/state/mod.rs`の`request_merge_worktree_branch()`で条件A（view_mode）に警告メッセージ追加
- [ ] 同上、条件B（worktrees empty/cursor out of range）に警告メッセージ追加（空とカーソル範囲外を分離）
- [ ] 手動テスト: Worktrees Viewで条件を満たさない状態でMキーを押して警告メッセージが表示されることを確認

## Phase 4: 差分チェック機能の追加

- [ ] `src/vcs/git/commands.rs`に`count_commits_ahead()`関数を追加（`git rev-list --count <base>..<branch>`）
- [ ] 単体テスト: `count_commits_ahead()`が正しくコミット数を返すことを確認
- [ ] `src/tui/types.rs`の`WorktreeInfo`に`has_commits_ahead: bool`フィールド追加
- [ ] `WorktreeInfo::display_*`メソッドや既存テストを更新（新フィールド対応）

## Phase 5: worktreeロード時の差分チェック

- [ ] `src/tui/runner.rs`の`load_worktrees_with_conflict_check()`で各worktreeの差分をチェック
- [ ] 並列実行: conflict checkと同様にJoinSetで並列化
- [ ] `has_commits_ahead`フィールドを設定してWorktreeInfoを構築
- [ ] 手動テスト: worktreeリストで差分状態が正しく検出されることを確認

## Phase 6: Mキー表示条件の厳密化

- [ ] `src/tui/render.rs`の`render_footer_worktree()`でMキー表示条件に`wt.has_commits_ahead`を追加
- [ ] `src/tui/state/mod.rs`の`request_merge_worktree_branch()`に`has_commits_ahead`チェック追加
- [ ] 警告メッセージ: "Cannot merge: no commits ahead of base branch"
- [ ] 手動テスト: baseと同じコミットのworktreeでMキーが非表示になることを確認
- [ ] 手動テスト: baseより先のコミットがあるworktreeでMキーが表示されることを確認

## Phase 7: 統合テスト

- [ ] 各条件でMキーの表示/非表示が正しく動作することを確認
- [ ] Mキー押下時のエラーメッセージが適切に表示されることを確認
- [ ] マージ可能なworktreeでMキーが正常に動作することを確認
- [ ] TUIがクラッシュせずに安定動作することを確認
- [ ] デバッグログで処理の流れが追跡可能なことを確認
