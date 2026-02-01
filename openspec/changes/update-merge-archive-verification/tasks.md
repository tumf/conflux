## 1. 実装
- [x] 1.1 `attempt_merge` の事前検証を `verify_archive_completion` から `is_archive_commit_complete` に置き換える
  - 検証: `src/parallel/mod.rs` の `attempt_merge` 関数で `is_archive_commit_complete` が呼ばれ、`verify_archive_completion` が使われていないことを確認する
- [x] 1.2 archive未完了時の理由メッセージを更新し、dirty/changes残存/archiveエントリ不在の文脈を含める
  - 検証: `src/parallel/mod.rs` の `MergeDeferred` 理由文字列に「archive」「incomplete」「dirty」のいずれかが含まれることを確認する

## 2. テスト
- [x] 2.1 worktreeがdirtyでarchive未完了の場合にmergeがdeferされるテストを追加する
  - 検証: `cargo test merge_deferred_when_worktree_dirty` が通る
- [x] 2.2 archiveエントリが存在しない場合にmergeがdeferされるテストを追加する
  - 検証: `cargo test merge_deferred_when_archive_entry_missing` が通る
- [x] 2.3 archive完了状態（clean + changes消失 + archiveエントリ存在）でmergeが実行されるテストを追加する
  - 検証: `cargo test merge_proceeds_when_archive_complete` が通る

## Acceptance #1 Failure Follow-up
- [x] src/parallel/tests/executor.rs: test_attempt_merge_defers_when_change_not_archived expects "Archive verification failed" but attempt_merge now returns "Archive incomplete"; update the assertion to match the new reason string
  - 検証: `cargo test test_attempt_merge_defers_when_change_not_archived` が通る
- [x] Git working tree is dirty. Modified: openspec/changes/update-merge-archive-verification/tasks.md
  - 検証: tasks.md の変更をコミットして `git status` がクリーンであることを確認する
