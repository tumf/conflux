## Context
server モードはプロジェクト追加時に `data_dir/<project_id>` に bare clone を作成し、`data_dir/worktrees/<project_id>/<branch>` に worktree を作る。現状は base ブランチ（例: `main`）をそのまま worktree に checkout するため、base 側で `refs/heads/<branch>` を更新する `git/pull` が拒否される。

## Goals / Non-Goals
- Goals:
  - server モードの worktree は base ブランチを checkout しない
  - base（bare）での `git/pull` / `git/push` が拒否されない
  - 非サーバモードと同様に「別ブランチの worktree」を作る
- Non-Goals:
  - worktree 作成・削除の外部 API を追加する
  - base 側の fetch/push 戦略の変更

## Decisions
- Decision: worktree ブランチ名は `server-wt/<project_id>/<base_branch>` とする
  - 理由: 一意性があり、一覧で識別しやすい
- Decision: 既存プロジェクトの worktree が base ブランチを checkout している場合は、再作成を促す
  - 理由: 既存 worktree の再利用は branch 競合のリスクが高いため

## Risks / Trade-offs
- 既存プロジェクトの worktree を作り直す必要がある
  - Mitigation: 明示的なエラーメッセージと再作成ガイドを提供する

## Migration Plan
- 既存 worktree が `refs/heads/<base_branch>` を checkout している場合、`git worktree remove` + 再作成を案内
- 新規プロジェクト追加時は常に server 専用ブランチで worktree を作成

## Open Questions
- なし
