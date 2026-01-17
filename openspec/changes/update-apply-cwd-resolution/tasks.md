## 1. 作業ディレクトリの統一
- [ ] 1.1 共通 apply/archive ループに `workspace_path`（任意）を渡せるようにする
- [ ] 1.2 parallel 側から worktree パスを必ず渡すようにする
- [ ] 1.3 serial 側は `workspace_path` を省略し、従来の repo root 実行を維持する

## 2. 環境変数/フックの整合
- [ ] 2.1 parallel 実行時に `OPENSPEC_WORKSPACE_PATH` などの文脈を維持する
- [ ] 2.2 hook 実行順序とイベント通知が変わらないことを確認する

## 3. 検証
- [ ] 3.1 parallel apply が worktree 内で実行されることを確認する
- [ ] 3.2 `cargo test` を実行して回帰がないことを確認する
