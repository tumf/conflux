# Tasks: Remove Hardcoded "main" Branch References

## Implementation Tasks

- [x] 1. `src/vcs/git/mod.rs` の `merge_branches()` 関数を修正
  - `unwrap_or("main")` を `ok_or_else()` に変更してエラーを返す
  - エラーメッセージ: "Original branch not initialized"

- [x] 2. `src/execution/state.rs` に `base_branch` パラメータを追加
  - `detect_workspace_state()` に `base_branch: &str` パラメータを追加
  - `is_merged_to_main()` を `is_merged_to_base()` にリネームし、`base_branch` パラメータを追加
  - ハードコードされた `"main"` をパラメータに置き換え

- [x] 3. `src/parallel/mod.rs` の呼び出し側を修正
  - `detect_workspace_state()` 呼び出し時に `original_branch` を渡す
  - `unwrap_or_else(|| "main".to_string())` を `ok_or_else()` に変更してエラーを返す

- [x] 4. テストコードの修正
  - `src/vcs/git/mod.rs` のテストで `"main"` を変数に置き換え
  - `src/execution/state.rs` のテストで `base_branch` パラメータを追加
  - `src/parallel/mod.rs` のテストで動的なブランチ名を使用

- [x] 5. エラーメッセージの一貫性確認
  - すべてのエラーメッセージが明確で一貫性があることを確認
  - ユーザーに対して適切なガイダンスを提供

- [x] 6. ドキュメント更新
  - AGENTS.md の関連セクションを更新（必要に応じて）
  - ベースブランチが動的に取得されることを明記

## Validation Tasks

- [x] 7. ユニットテストの実行
  - `cargo test` が全て成功することを確認

- [x] 8. 異なるブランチからの実行テスト
  - `main` ブランチから実行
  - `develop` ブランチから実行（テストリポジトリで）
  - フィーチャーブランチから実行（テストリポジトリで）

- [x] 9. 並列実行モードのテスト
  - Git worktree を使った並列実行が正常に動作することを確認
  - マージ処理が正しいベースブランチに対して行われることを確認

- [x] 10. エラーハンドリングのテスト
  - `original_branch` が未初期化の場合に適切なエラーが返されることを確認

- [x] 11. Clippy とフォーマットチェック
  - `cargo fmt --check` が成功することを確認
  - `cargo clippy -- -D warnings` が成功することを確認
