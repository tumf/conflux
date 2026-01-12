# Tasks: マージコミットを empty に保つ

## 1. `workspace update-stale` の削除

- [ ] 1.1 `src/vcs/jj/mod.rs` の `merge_jj_workspaces()` メソッドから `workspace update-stale` の呼び出しを削除（行 365-373）
- [ ] 1.2 関連するコメントも更新（"Refresh working copy..." のコメントを削除）

## 2. 新しい working copy コミットの作成

- [ ] 2.1 `merge_jj_workspaces()` メソッドで、マージコミット作成後に `jj new <merge_rev>` を実行
- [ ] 2.2 `jj new` の実行結果を確認し、エラーハンドリングを追加
- [ ] 2.3 `jj new` で作成された新しいコミットの change_id を取得（必要に応じて）
- [ ] 2.4 関連するログメッセージを追加（debug レベル）

## 3. 関連箇所の確認と修正

- [ ] 3.1 `src/parallel/mod.rs` で `merge_and_resolve()` の呼び出し箇所を確認
- [ ] 3.2 `workspace update-stale` が他の場所で使われていないか確認（`rg "workspace update-stale"` で検索）
- [ ] 3.3 他の箇所で使われている場合、同様の問題がないか確認し、必要に応じて修正

## 4. テストの追加・更新

- [ ] 4.1 `src/vcs/jj/mod.rs` のテストセクションで、マージコミットが empty であることを検証するテストを追加
- [ ] 4.2 既存のマージ関連テストが正しく動作することを確認
- [ ] 4.3 テストで `jj log -r <merge_rev> -T 'empty'` を使用して empty 状態を確認

## 5. 統合テストと検証

- [ ] 5.1 並列実行モードで複数の変更を処理し、マージコミットが empty であることを確認
  ```bash
  cargo build && cargo run -- run --parallel --dry-run
  jj log -r @ -T 'change_id ++ " " ++ if(empty, "(empty)", "(non-empty)")' 
  ```
- [ ] 5.2 マージコミットの後に作成される working copy コミットが存在することを確認
- [ ] 5.3 コンフリクトが発生した場合でも正しく動作することを確認
- [ ] 5.4 単一の変更のマージでも正しく動作することを確認

## 6. ドキュメントの更新

- [ ] 6.1 コードコメントで、マージコミットが empty であることを明記
- [ ] 6.2 `merge_jj_workspaces()` メソッドの docstring を更新し、動作を正確に記述
- [ ] 6.3 必要に応じて AGENTS.md にマージ処理のパターンを記載

## 7. リリース準備

- [ ] 7.1 `cargo fmt` でフォーマットを整える
- [ ] 7.2 `cargo clippy -- -D warnings` でリントチェック
- [ ] 7.3 `cargo test` で全テストが通ることを確認
- [ ] 7.4 変更内容をコミットメッセージに記載（"fix: Ensure merge commits are always empty in jj"）
