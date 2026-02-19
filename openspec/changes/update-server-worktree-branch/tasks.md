## 1. server worktree ブランチ化

- [x] 1.1 server 用 worktree ブランチ名の生成ルールを追加する（`src/server/registry.rs` に `server_worktree_branch(project_id, base_branch)` 関数を実装し `server-wt/<project_id>/<base_branch>` 形式で生成）
- [x] 1.2 `POST /api/v1/projects` の worktree 作成が server 専用ブランチを使うように変更する（`src/server/api.rs` の worktree 作成で `git worktree add -b <server-branch>` を使用）

## 2. 既存プロジェクトの取り扱い

- [x] 2.1 既存 worktree が base ブランチを checkout している場合の検知とエラーを追加する（`src/server/api.rs` で `git worktree list --porcelain` の出力を解析して `refs/heads/<base>` を検知して 409 Conflict エラーを返す）
- [x] 2.2 エラーメッセージに再作成手順を含める（DELETE /api/v1/projects/:id して再登録する手順がレスポンスに含まれる）

## 3. テスト

- [x] 3.1 server 用 worktree ブランチ名生成の単体テストを追加する（`src/server/registry.rs` の `#[cfg(test)]` モジュールに5件のテストを追加し `server-wt/<project_id>/<base_branch>` 形式になることを確認）
- [x] 3.2 `POST /api/v1/projects` が server 専用ブランチで worktree を作成することを確認するテストを追加する（`src/server/api.rs` の `test_add_project_creates_worktree_on_server_branch` テストで worktree の HEAD が base ブランチ以外であることを確認）
