# Change: サーバの worktree を別ブランチで作成する

## Why
server モードの git/pull/push を base（bare）で行う前提では、worktree が `main` を checkout していると base の `refs/heads/main` 更新が拒否されるため。

## What Changes
- `POST /api/v1/projects` で作成する worktree は、base ブランチとは別のブランチ名で作成する
- worktree ブランチ名は server 専用の命名規則（project_id と base branch を含む）で生成する
- 既存プロジェクトの worktree が base ブランチを checkout している場合の移行手順を用意する

## Impact
- Affected specs: `openspec/specs/server-mode/spec.md`
- Affected code: `src/server/api.rs`, `src/server/registry.rs`
