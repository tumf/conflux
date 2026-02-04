## 1. Implementation
- [x] 1.1 change 単位の未コミット検出（`openspec/changes/<change_id>/` 配下の staged/unstaged/untracked）を抽出するヘルパーを追加する
  - 検証: ヘルパーが `openspec/changes/<change_id>` の差分だけを判定し、該当 change ID を返すことを `src/vcs/git/commands/commit.rs` で確認
- [x] 1.2 並列実行の対象判定に「部分未コミット change を除外する」条件を組み込む
  - 検証: `src/parallel_run_service.rs` の `filter_committed_changes()` が change 単位の未コミット集合を除外していることを確認
- [x] 1.3 TUI の `UNCOMMITED` 判定に change 単位の未コミット検出を反映する
  - 検証: `src/tui/runner.rs`（初期/refresh）と `src/tui/orchestrator.rs`（並列開始時）で、更新後の committed change 集合が使われていることを確認
- [x] 1.4 change 配下に差分がある場合の回帰テストを追加する
  - 検証: `cargo test` で対象テストが通ること

## 2. Validation
- [x] 2.1 `npx @fission-ai/openspec@latest validate update-uncommitted-change-detection --strict` を実行する
  - 検証: エラーが出ないこと
