## Context
server モードの worktree は `data_dir/worktrees/<project_id>/<branch>` に作成される。現状は base ブランチをそのまま checkout しており、base（bare）で `refs/heads/<branch>` を更新する `git/pull` が拒否される。

## Design
- worktree は server 専用ブランチ（`server-wt/<project_id>/<base_branch>`）で作成する
- base の `refs/heads/<base_branch>` は worktree から切り離され、base の `git/pull` が許可される
