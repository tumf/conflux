## 1. server worktree ブランチ化

- [ ] 1.1 server 用 worktree ブランチ名の生成ルールを追加する（`src/server/registry.rs` などに `server-wt/<project_id>/<base_branch>` 生成が実装されていることを確認）
- [ ] 1.2 `POST /api/v1/projects` の worktree 作成が server 専用ブランチを使うように変更する（`src/server/api.rs` の worktree 作成が `-b <server-branch>` を使うことを確認）

## 2. 既存プロジェクトの取り扱い

- [ ] 2.1 既存 worktree が base ブランチを checkout している場合の検知とエラーを追加する（`src/server/api.rs` で `refs/heads/<base>` を検知して明示エラーになることを確認）
- [ ] 2.2 エラーメッセージに再作成手順を含める（worktree 削除と再追加の手順がレスポンスに含まれることを確認）

## 3. テスト

- [ ] 3.1 server 用 worktree ブランチ名生成の単体テストを追加する（`server-wt/<project_id>/<base_branch>` 形式になることを確認）
- [ ] 3.2 `POST /api/v1/projects` が server 専用ブランチで worktree を作成することを確認するテストを追加する（worktree の branch が base 以外であることを確認）
