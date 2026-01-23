## 1. Implementation
- [x] 1.1 worktree の archive 判定を、コミットメッセージではなくファイル状態（clean 状態、`openspec/changes/<change_id>` 不在、archive エントリ存在）で判定できるよう更新する（`src/execution/archive.rs`, `src/execution/state.rs`）。確認方法: `detect_workspace_state` が更新後の判定ヘルパーを参照していることを確認する。
- [x] 1.2 `WorkspaceState::Archived` で再開した場合に acceptance/archiving を再実行せず merge へ進むことを保証する（`src/parallel/mod.rs`）。確認方法: `WorkspaceState::Archived` 分岐が merge-only のフローに留まっていることを確認する。
- [x] 1.3 ファイル状態ベースで archived と判定されるテストを追加する（`src/execution/state.rs`）。確認方法: 新規テスト名と期待値を確認する。

## 2. Validation
- [x] 2.1 `cargo test test_detect_workspace_state_archived` を実行し、archive 判定テストが通ることを確認する。

## Acceptance #1 Failure Follow-up
- [x] `src/execution/state.rs` の `is_merged_to_base()` が `git log --grep "Archive: <change_id>"` に依存して merged 判定しており、base ブランチの HEAD ツリーにおける archive エントリ存在/`openspec/changes/<change_id>` 不在のファイル状態判定になっていない。
- [x] `src/execution/state.rs` の `has_archive_files()` が archive エントリの存在のみを確認しており、archiving 判定に必要な worktree dirty と `openspec/changes/<change_id>` 不在の条件を検証していない。

## Acceptance #2 Failure Follow-up
- [x] `src/parallel/mod.rs` の `execute_changes_dispatch()` で `WorkspaceState::Archiving` の acceptance が `Continue`/`Fail`/`CommandFailed`/`Err` の場合に `changes_for_apply.push(...)` で apply へ戻しており、仕様の「archiving の場合は apply を再実行せず archive ループに進む」に違反している。修正: Archiving状態でaccceptanceが失敗した場合は、applyに戻すのではなく、エラーとして扱いワークスペースを保持、failed_trackerにマークして依存changeをスキップするように変更。
