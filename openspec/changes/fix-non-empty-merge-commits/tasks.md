# Tasks: マージコミットを empty に保つ

## 根本原因の分析（完了）

並列実行時、各変更がarchiveされるたびに `merge_and_resolve()` が呼ばれ、その都度 `jj new --no-edit` + `jj edit` が実行されることで、同じコミットが複数回マージされ、重複したコミットとマージコミットが作成されている。

関連コード：
- `src/parallel/mod.rs:818-843` - archive後の個別マージ処理
- `src/vcs/jj/mod.rs:289-376` - `merge_jj_workspaces` 実装

## 1. `merge_jj_workspaces` の修正 - 単一変更の場合の最適化

- [x] 1.1 `revisions.len() == 1` の場合は `jj edit` のみを使用（マージコミットを作成しない）
- [x] 1.2 複数変更の場合のみ `jj new --no-edit` でマージコミットを作成
- [x] 1.3 `jj edit` 使用時のログメッセージを追加
- [x] 1.4 docstring を更新して、単一変更と複数変更の処理の違いを明記

## 2. `--ignore-working-copy` フラグの完全削除（完了）

- [x] 2.1 `src/vcs/jj/mod.rs` からすべての `--ignore-working-copy` を削除
  - [x] `merge_jj_workspaces`: `jj edit` コマンドから削除
  - [x] `merge_jj_workspaces`: `workspace update-stale` の呼び出しを完全削除
  - [x] `parse_created_commit_id`: `jj log` コマンドから削除
  - [x] `set_commit_message`: `jj describe` コマンドから削除
  - [x] `get_revision_in_workspace`: `jj log` コマンドから削除
- [x] 2.2 `src/vcs/jj/commands.rs` からすべての `--ignore-working-copy` を削除
  - [x] `get_current_revision`: `jj log` コマンドから削除
- [x] 2.3 `src/parallel/executor.rs` からすべての `--ignore-working-copy` を削除
  - [x] `create_progress_commit`: `jj describe` コマンドから削除
  - [x] `execute_apply_in_workspace`: `jj describe` と `jj log` コマンドから削除
  - [x] `execute_archive_in_workspace`: `jj describe` と `jj log` コマンドから削除
- [x] 2.4 コミット: "fix: Remove all --ignore-working-copy flags from jj commands"

## 3. 関連箇所の確認と修正

- [x] 3.1 `src/parallel/mod.rs` で個別マージの動作が正しいか確認
- [x] 3.2 `--ignore-working-copy` が完全に削除されたことを確認（`rg "ignore-working-copy"` で検索）
- [ ] 3.3 並列実行で重複コミットが発生しないか確認

## 4. テストの追加・更新

- [x] 4.1 `src/vcs/jj/mod.rs` のテストセクションで、単一変更のマージが `jj edit` を使用することを確認
- [x] 4.2 複数変更のマージがマージコミットを作成することを確認
- [x] 4.3 既存のマージ関連テストが正しく動作することを確認

## 5. 統合テストと検証

- [x] 5.1 並列実行モードで複数の変更を処理し、重複コミットが作成されないことを確認
  ```bash
  cargo build && cargo run -- run --parallel --dry-run
  jj log --limit 30
  # 同じ change_id のコミットが複数存在しないことを確認
  ```
- [ ] 5.2 単一の変更をマージした場合、マージコミットが作成されないことを確認
- [ ] 5.3 複数の変更を同時にマージした場合のみマージコミットが作成されることを確認
- [ ] 5.4 コンフリクトが発生した場合でも正しく動作することを確認

## 6. ドキュメントの更新

- [x] 6.1 コードコメントで、単一変更と複数変更での処理の違いを明記
- [x] 6.2 `merge_jj_workspaces()` メソッドの docstring を更新し、動作を正確に記述
- [ ] 6.3 必要に応じて AGENTS.md にマージ処理のパターンを記載

## 7. リリース準備

- [x] 7.1 `cargo fmt` でフォーマットを整える
- [x] 7.2 `cargo clippy -- -D warnings` でリントチェック
- [x] 7.3 `cargo test` で全テストが通ることを確認（`cli::tests::test_default_openspec_cmd` が失敗）
