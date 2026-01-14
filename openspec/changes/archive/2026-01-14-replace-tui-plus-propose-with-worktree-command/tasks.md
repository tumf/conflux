# Tasks

- [x] `worktree_command` を設定スキーマとして追加し、設定ファイルから読み込めるようにする
- [x] `worktree_command` のプレースホルダー展開（`{workspace_dir}` / `{repo_root}`）とシェルエスケープ規則を定義・実装する
- [x] TUIの `+` ハンドラを `worktree_command` フローに置き換える（Proposingモードは起動しない）
- [x] Gitリポジトリ検出と無操作条件（git上でない / `worktree_command` 未設定）を実装する
- [x] 一時ディレクトリ配下に Git worktree を作成する処理を追加する（削除せず残す）
- [x] `worktree_command` を worktree の `cwd` で実行し、開始/終了/失敗をログに反映する
- [x] Proposingモード関連の状態・イベント・レンダリング・キーヒントを削除または無効化する
- [x] 既存のTUIユニットテストを更新し、`+` の新挙動（無操作条件含む）をテストで担保する
- [x] `cargo test` を実行して関連テストが通ることを確認する
- [x] `npx @fission-ai/openspec@latest validate replace-tui-plus-propose-with-worktree-command --strict` を実行し、提案が検証を通ることを確認する
